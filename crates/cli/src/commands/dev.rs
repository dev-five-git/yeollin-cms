//! Dev command
//!
//! Runs prebuild, then starts Next.js dev server and Rust API server.
//! Expects to be run from a directory containing api/ and/or app/ subdirectories.

use std::process::Stdio;
use clap::Args;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, debug};

use super::prebuild::{detect_current_app, detect_crate_dir, run_prebuild, find_binary_path, export_from_binary};

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
        run_prebuild(&yeollin_app_dir, frontend.as_ref(), menus_json.as_deref(), plugins_json.as_deref(), false).await?;
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
        info!("  Frontend: proxied from internal port {}", args.internal_frontend_port);
    }

    // 5. Start Next.js dev server if we have frontend
    let mut next_handle = if frontend.is_some() {
        let mut next_cmd = Command::new("bun");
        next_cmd
            .current_dir(&yeollin_app_dir)
            .args(["run", "dev", "--port", &args.internal_frontend_port.to_string()])
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
