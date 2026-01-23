//! Build command
//!
//! Runs prebuild, then builds Next.js for static export and Rust binary.
//! Expects to be run from a directory containing api/ and/or app/ subdirectories.

use anyhow::{Context, Result};
use clap::Args;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use super::prebuild::{
    detect_crate_dir, detect_current_app, export_from_binary, find_binary_path, run_prebuild,
};

#[derive(Args)]
pub struct BuildArgs {
    /// Skip prebuild step
    #[arg(long)]
    pub skip_prebuild: bool,

    /// Build in release mode
    #[arg(long, default_value = "true")]
    pub release: bool,

    /// Skip frontend build
    #[arg(long)]
    pub skip_frontend: bool,

    /// Skip backend build
    #[arg(long)]
    pub skip_backend: bool,
}

pub async fn run(args: BuildArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let crate_dir = detect_crate_dir();
    let frontend = detect_current_app();

    info!("Build starting...");
    info!("  Current dir: {}", current_dir.display());
    info!("  Has crate:   {}", crate_dir.is_some());
    info!("  Has app/:    {}", frontend.is_some());

    if crate_dir.is_none() && frontend.is_none() {
        anyhow::bail!("No Cargo.toml or app/ directory found. Run from an app/plugin directory.");
    }

    let yeollin_app_dir = current_dir.join(".yeollin").join("app");

    // 1. Build crate first (debug mode for menu export)
    if let Some(ref crate_path) = crate_dir {
        info!("Building crate (debug)...");
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
        let binary_path = find_binary_path(crate_path).await?;

        info!("Exporting menus from binary...");
        let menus = match export_from_binary(&binary_path, "YEOLLIN_EXPORT_MENUS").await {
            Ok(m) => {
                info!("Exported menus successfully");
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
                info!("Exported plugins successfully");
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
    if !args.skip_prebuild && frontend.is_some() {
        info!("Running prebuild...");
        run_prebuild(
            &yeollin_app_dir,
            frontend.as_ref(),
            menus_json.as_deref(),
            plugins_json.as_deref(),
            true,
        )
        .await?;
    }

    // 4. Install dependencies and build frontend
    if !args.skip_frontend && frontend.is_some() {
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

        info!("Building frontend (Next.js SSG)...");
        let status = Command::new("bun")
            .current_dir(&yeollin_app_dir)
            .args(["run", "build"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .context("Failed to run next build")?;

        if !status.success() {
            anyhow::bail!("Next.js build failed");
        }

        info!(
            "Frontend build complete: {}",
            yeollin_app_dir.join("out").display()
        );
    }

    // 5. Build Rust binary in release mode
    if !args.skip_backend {
        if let Some(ref crate_path) = crate_dir {
            info!("Building backend (Rust release)...");

            let mut cargo_args = vec!["build"];
            if args.release {
                cargo_args.push("--release");
            }

            let status = Command::new("cargo")
                .current_dir(crate_path)
                .args(&cargo_args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await
                .context("Failed to run cargo build")?;

            if !status.success() {
                anyhow::bail!("Cargo build failed");
            }

            let profile = if args.release { "release" } else { "debug" };
            info!("Backend build complete: target/{}/", profile);
        }
    }

    info!("Build complete!");
    Ok(())
}
