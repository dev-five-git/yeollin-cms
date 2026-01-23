//! Dev command
//!
//! Runs prebuild, then starts Next.js dev server and Rust API server.
//! Watches for file changes in app/ and syncs to .yeollin/app/src/app/.
//! Expects to be run from a directory containing api/ and/or app/ subdirectories.

use anyhow::{Context, Result};
use clap::Args;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;
use tokio::process::Command;
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
}

pub async fn run(args: DevArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let crate_dir = detect_crate_dir();
    let frontend = detect_current_app();

    info!("Development server starting...");
    info!("  Current dir: {}", current_dir.display());
    info!("  Has crate:   {}", crate_dir.is_some());
    info!("  Has app/:    {}", frontend.is_some());

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

    // 2. Export menus and plugins from binary (after build)
    let (menus_json, plugins_json) = if let Some(ref crate_path) = crate_dir {
        let binary_path = find_binary_path(crate_path)?;

        info!("Exporting menus from binary...");
        let menus = match export_from_binary(&binary_path, "YEOLLIN_EXPORT_MENUS").await {
            Ok(m) => {
                info!("Exported menus: {}", m);
                Some(m)
            }
            Err(e) => {
                debug!("Could not export menus: {}", e);
                None
            }
        };

        info!("Exporting plugins from binary...");
        let plugins = match export_from_binary(&binary_path, "YEOLLIN_EXPORT_PLUGINS").await {
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
    }

    // 5. Start file watcher for hot reload
    let watcher_handle = if let Some(ref app) = frontend {
        info!("Starting file watcher for hot reload...");
        Some(start_file_watcher(app, yeollin_app_dir.clone())?)
    } else {
        None
    };

    // 7. Start Next.js dev server if we have frontend
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

    // 8. Start API server
    let mut cargo_handle = if let Some(ref crate_path) = crate_dir {
        let mut cargo_cmd = Command::new("cargo");
        cargo_cmd
            .current_dir(crate_path)
            .args(["run"])
            .env("PORT", args.port.to_string());

        // Enable dev proxy if we have frontend
        if frontend.is_some() {
            cargo_cmd.env("YEOLLIN_DEV_PROXY", args.internal_frontend_port.to_string());
        }

        cargo_cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);

        Some(cargo_cmd.spawn()?)
    } else {
        None
    };

    info!("Development server started. Press Ctrl+C to stop.");

    // Wait for processes to exit (watcher runs until other processes exit)
    match (next_handle.as_mut(), cargo_handle.as_mut()) {
        (Some(next), Some(cargo)) => {
            tokio::select! {
                result = next.wait() => {
                    if let Ok(status) = result {
                        if !status.success() {
                            tracing::error!("Next.js dev server exited with status: {}", status);
                        }
                    }
                    let _ = cargo.kill().await;
                }
                result = cargo.wait() => {
                    if let Ok(status) = result {
                        if !status.success() {
                            tracing::error!("API server exited with status: {}", status);
                        }
                    }
                    let _ = next.kill().await;
                }
            }
        }
        (Some(next), None) => {
            let _ = next.wait().await;
        }
        (None, Some(cargo)) => {
            let _ = cargo.wait().await;
        }
        (None, None) => {}
    }

    // Watcher handle will be dropped when function exits, stopping the watcher
    drop(watcher_handle);

    info!("Development server stopped.");
    Ok(())
}

/// Strip all route groups (parenthesized segments) from a path
/// e.g., "aa/bb/(public)/cc/page.tsx" -> "aa/bb/cc/page.tsx"
fn strip_route_groups(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            // Skip route groups like (public), (dashboard), etc.
            if !(name_str.starts_with('(') && name_str.ends_with(')')) {
                result.push(name);
            }
        }
    }
    result
}

/// Sync a single file from app/ to .yeollin/app/src/app/
/// Determines if it's a public or auth route based on (public) marker in path
fn sync_file(src_path: &Path, app_path: &Path, yeollin_app_dir: &Path) -> Result<()> {
    // Get relative path from app directory
    let rel_path = match src_path.strip_prefix(app_path) {
        Ok(p) => p,
        Err(_) => {
            debug!("File not under app path: {}", src_path.display());
            return Ok(());
        }
    };

    let rel_str = rel_path.to_string_lossy();

    // Check if (public) appears anywhere in the path
    let is_public = rel_str.contains("(public)");

    // Build clean path by stripping all route groups
    let clean_path = strip_route_groups(rel_path);

    // Determine destination
    let dest_path = if is_public {
        yeollin_app_dir
            .join("src")
            .join("app")
            .join("(public)")
            .join(&clean_path)
    } else {
        yeollin_app_dir
            .join("src")
            .join("app")
            .join("(auth)")
            .join(&clean_path)
    };

    // Handle file operations
    if src_path.exists() && src_path.is_file() {
        // Create parent directories and copy file
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src_path, &dest_path)?;
        info!(
            "Synced {} -> {} ({})",
            rel_str,
            dest_path.display(),
            if is_public { "public" } else { "auth" }
        );
    } else if !src_path.exists() {
        // File was deleted, remove from destination
        if dest_path.exists() {
            fs::remove_file(&dest_path)?;
            info!("Removed {}", dest_path.display());
        }
    }

    Ok(())
}

/// Start file watcher for app/ directory
/// Returns a JoinHandle that runs the watcher in a blocking task
fn start_file_watcher(
    frontend: &AppFrontend,
    yeollin_app_dir: PathBuf,
) -> Result<tokio::task::JoinHandle<()>> {
    let app_path = frontend.app_path.clone();

    let handle = tokio::task::spawn_blocking(move || {
        let (tx, rx) = mpsc::channel::<DebounceEventResult>();

        let mut debouncer = match new_debouncer(Duration::from_millis(500), tx) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to create file watcher: {}", e);
                return;
            }
        };

        if let Err(e) = debouncer.watcher().watch(&app_path, RecursiveMode::Recursive) {
            warn!("Failed to watch directory {}: {}", app_path.display(), e);
            return;
        }

        info!("File watcher started for {}", app_path.display());

        // Process file change events
        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    for event in events {
                        let path = &event.path;

                        // Skip non-file events and hidden files
                        if path.is_dir() {
                            continue;
                        }
                        if path
                            .file_name()
                            .map(|n| n.to_string_lossy().starts_with('.'))
                            .unwrap_or(false)
                        {
                            continue;
                        }

                        // Sync the changed file
                        if let Err(e) = sync_file(path, &app_path, &yeollin_app_dir) {
                            warn!("Failed to sync {}: {}", path.display(), e);
                        }
                    }
                }
                Ok(Err(error)) => {
                    warn!("Watch error: {}", error);
                }
                Err(e) => {
                    // Channel closed, watcher is done
                    debug!("File watcher channel closed: {}", e);
                    break;
                }
            }
        }
    });

    Ok(handle)
}
