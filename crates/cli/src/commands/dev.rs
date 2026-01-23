//! Dev command
//!
//! Runs prebuild (proxy mode), then starts Next.js dev server and Rust API server.
//! Proxy mode creates re-export files that import from the original source,
//! enabling instant HMR. A file watcher detects new/deleted files and updates proxies,
//! and restarts the Rust server to pick up new public routes.

use anyhow::{Context, Result};
use clap::Args;
use notify::{event::ModifyKind, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use tokio::process::{Child, Command};
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, info, warn};

use super::prebuild::{
    detect_crate_dir, detect_current_app, export_from_binary, find_binary_path, run_prebuild,
    AppFrontend,
};

#[derive(Args)]
pub struct DevArgs {
    /// Skip prebuild step
    #[arg(long)]
    pub skip_prebuild: bool,

    /// Main CMS port (single entry point)
    #[arg(long, default_value = "3001")]
    pub port: u16,

    /// Internal port for Next.js dev server (proxied through main port)
    #[arg(long, default_value = "3000")]
    pub internal_frontend_port: u16,

    /// Use copy mode instead of proxy mode (disables instant HMR)
    #[arg(long)]
    pub copy_mode: bool,
}

pub async fn run(args: DevArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let crate_dir = detect_crate_dir();
    let frontend = detect_current_app();

    info!("Development server starting...");
    info!("  Current dir: {}", current_dir.display());
    info!("  Has crate:   {}", crate_dir.is_some());
    info!("  Has app/:    {}", frontend.is_some());
    info!(
        "  Mode:        {}",
        if args.copy_mode {
            "copy"
        } else {
            "proxy (instant HMR)"
        }
    );

    if crate_dir.is_none() && frontend.is_none() {
        anyhow::bail!("No Cargo.toml or app/ directory found. Run from an app/plugin directory.");
    }

    // 1. Build crate if exists
    if let Some(ref crate_path) = crate_dir {
        info!("Building crate...");
        let build_status = Command::new("cargo")
            .current_dir(crate_path)
            .args(["build"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?;

        if !build_status.success() {
            anyhow::bail!("Failed to build crate");
        }
    }

    // 2. Export menus and plugins from binary (after build) IN PARALLEL
    let (menus_json, plugins_json) = if let Some(ref crate_path) = crate_dir {
        let binary_path = find_binary_path(crate_path).await?;

        info!("Exporting menus and plugins from binary (parallel)...");

        let (menus_result, plugins_result) = tokio::join!(
            export_from_binary(&binary_path, "YEOLLIN_EXPORT_MENUS"),
            export_from_binary(&binary_path, "YEOLLIN_EXPORT_PLUGINS")
        );

        let menus = match menus_result {
            Ok(m) => {
                info!("Exported menus: {}", m);
                Some(m)
            }
            Err(e) => {
                debug!("Could not export menus: {}", e);
                None
            }
        };

        let plugins = match plugins_result {
            Ok(p) => {
                info!("Exported plugins: {}", p);
                Some(p)
            }
            Err(e) => {
                debug!("Could not export plugins: {}", e);
                None
            }
        };

        (menus, plugins)
    } else {
        (None, None)
    };

    // 3. Run prebuild if not skipped and we have frontend
    let yeollin_app_dir = current_dir.join(".yeollin").join("app");

    if !args.skip_prebuild && frontend.is_some() {
        info!("Running prebuild...");
        run_prebuild(
            &yeollin_app_dir,
            frontend.as_ref(),
            menus_json.as_deref(),
            plugins_json.as_deref(),
            false,
            !args.copy_mode, // use_proxy = true by default (unless --copy-mode)
        )
        .await?;
    }

    // 4. Install frontend dependencies if needed
    if frontend.is_some() && !yeollin_app_dir.join("node_modules").exists() {
        info!("Installing frontend dependencies...");
        let install_status = Command::new("bun")
            .current_dir(&yeollin_app_dir)
            .args(["install"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .context("Failed to run bun install")?;

        if !install_status.success() {
            anyhow::bail!("bun install failed");
        }
    }

    info!("Starting development server (single port mode)...");
    info!("  API:      http://localhost:{}", args.port);
    if frontend.is_some() {
        info!(
            "  Frontend: proxied from internal port {}",
            args.internal_frontend_port
        );
        if !args.copy_mode {
            info!("  HMR:      instant (proxy mode - edit app/ files directly)");
        }
    }

    // 5. Start Next.js dev server if we have frontend
    let mut next_handle = if frontend.is_some() {
        let mut next_cmd = Command::new("bun");
        next_cmd
            .current_dir(&yeollin_app_dir)
            .args([
                "run",
                "dev",
                "--port",
                &args.internal_frontend_port.to_string(),
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        Some(next_cmd.spawn()?)
    } else {
        None
    };

    // Wait for Next.js to be ready
    if next_handle.is_some() {
        info!("Waiting for Next.js to start...");
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }

    // 6. Start API server
    let mut cargo_handle = if let Some(ref crate_path) = crate_dir {
        Some(
            start_rust_server(
                crate_path,
                args.port,
                frontend.is_some().then_some(args.internal_frontend_port),
            )
            .await?,
        )
    } else {
        None
    };

    info!("Development server started. Press Ctrl+C to stop.");

    // 7. Start file watcher for proxy mode (with restart channel)
    let (restart_tx, mut restart_rx) = tokio_mpsc::channel::<()>(1);

    let watcher_handle = if !args.copy_mode && frontend.is_some() {
        let frontend_clone = frontend.clone().unwrap();
        let yeollin_app_dir_clone = yeollin_app_dir.clone();
        let menus_json_clone = menus_json.clone();
        let plugins_json_clone = plugins_json.clone();

        Some(tokio::spawn(async move {
            if let Err(e) = run_file_watcher(
                frontend_clone,
                yeollin_app_dir_clone,
                menus_json_clone,
                plugins_json_clone,
                restart_tx,
            )
            .await
            {
                warn!("File watcher error: {}", e);
            }
        }))
    } else {
        None
    };

    // Main event loop - handle process exits and restart signals
    loop {
        tokio::select! {
            // Handle Next.js exit
            result = async {
                match next_handle.as_mut() {
                    Some(h) => Some(h.wait().await),
                    None => std::future::pending().await,
                }
            } => {
                if let Some(Ok(status)) = result {
                    if !status.success() {
                        tracing::error!("Next.js dev server exited with status: {}", status);
                    }
                }
                if let Some(ref mut cargo) = cargo_handle {
                    let _ = cargo.kill().await;
                }
                break;
            }

            // Handle Rust server exit (without restart signal = real exit)
            result = async {
                match cargo_handle.as_mut() {
                    Some(h) => Some(h.wait().await),
                    None => std::future::pending().await,
                }
            } => {
                if let Some(Ok(status)) = result {
                    if !status.success() {
                        tracing::error!("API server exited with status: {}", status);
                    }
                }
                if let Some(ref mut next) = next_handle {
                    let _ = next.kill().await;
                }
                break;
            }

            // Handle restart signal from file watcher
            _ = restart_rx.recv() => {
                if let Some(ref crate_path) = crate_dir {
                    info!("Restarting Rust server to pick up route changes...");

                    // Kill current server
                    if let Some(ref mut cargo) = cargo_handle {
                        let _ = cargo.kill().await;
                    }

                    // Start new server
                    cargo_handle = Some(start_rust_server(
                        crate_path,
                        args.port,
                        frontend.is_some().then_some(args.internal_frontend_port),
                    ).await?);

                    info!("Rust server restarted");
                }
            }
        }
    }

    // Abort watcher if running
    if let Some(handle) = watcher_handle {
        handle.abort();
    }

    info!("Development server stopped.");
    Ok(())
}

/// Start the Rust API server
async fn start_rust_server(
    crate_path: &PathBuf,
    port: u16,
    frontend_port: Option<u16>,
) -> Result<Child> {
    let mut cargo_cmd = Command::new("cargo");
    cargo_cmd
        .current_dir(crate_path)
        .args(["run"])
        .env("PORT", port.to_string());

    // Enable dev proxy if we have frontend
    if let Some(frontend_port) = frontend_port {
        cargo_cmd.env("YEOLLIN_DEV_PROXY", frontend_port.to_string());
    }

    cargo_cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    Ok(cargo_cmd.spawn()?)
}

/// Watch app/ directory for file structure changes and update proxy files
async fn run_file_watcher(
    frontend: AppFrontend,
    output_dir: PathBuf,
    menus_json: Option<String>,
    plugins_json: Option<String>,
    restart_tx: tokio_mpsc::Sender<()>,
) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

    // Watch the app/ directory
    watcher.watch(&frontend.app_path, RecursiveMode::Recursive)?;
    info!(
        "Watching {} for file changes...",
        frontend.app_path.display()
    );

    // Process events with debouncing
    let mut last_rebuild = std::time::Instant::now();
    let debounce_duration = std::time::Duration::from_millis(500);

    loop {
        match rx.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(event) => {
                // React to create/remove/rename events
                let should_rebuild = matches!(
                    event.kind,
                    EventKind::Create(_)
                        | EventKind::Remove(_)
                        | EventKind::Modify(ModifyKind::Name(_))
                );

                if should_rebuild && last_rebuild.elapsed() > debounce_duration {
                    info!(
                        "File structure changed ({:?}), updating proxies...",
                        event.kind
                    );

                    // Re-run frontend linking (preserve menus and plugins)
                    if let Err(e) = run_prebuild(
                        &output_dir,
                        Some(&frontend),
                        menus_json.as_deref(),
                        plugins_json.as_deref(),
                        false,
                        true, // use_proxy
                    )
                    .await
                    {
                        warn!("Failed to update proxies: {}", e);
                    } else {
                        info!("Proxies updated successfully");

                        // Signal main loop to restart Rust server (for public route changes)
                        let _ = restart_tx.send(()).await;
                    }

                    last_rebuild = std::time::Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Continue waiting
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}
