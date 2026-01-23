//! Dev command
//!
//! Runs prebuild (proxy mode), then starts Next.js dev server and Rust API server.
//! Proxy mode creates re-export files that import from the original source,
//! enabling instant HMR without needing a file watcher.

use anyhow::{Context, Result};
use clap::Args;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use super::prebuild::{
    detect_crate_dir, detect_current_app, export_from_binary, find_binary_path, run_prebuild,
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

    // Wait for processes to exit
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

    info!("Development server stopped.");
    Ok(())
}
