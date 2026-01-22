//! Build command
//!
//! Runs prebuild, then builds Next.js for static export and Rust binary.

use std::path::PathBuf;
use std::process::Stdio;
use clap::Args;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::info;

use super::prebuild::{self, PrebuildArgs};

#[derive(Args)]
pub struct BuildArgs {
    /// Project root directory
    #[arg(short, long)]
    pub project_dir: Option<PathBuf>,

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
    let project_dir = args.project_dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    // 1. Run prebuild if not skipped
    if !args.skip_prebuild {
        info!("Running prebuild...");
        prebuild::run(PrebuildArgs {
            project_dir: Some(project_dir.clone()),
            output_dir: None,
            force: true,  // Force clean rebuild for production
        }).await?;
    }

    let yeollin_app_dir = project_dir.join(".yeollin").join("app");

    // 2. Build Next.js for static export
    if !args.skip_frontend {
        info!("Building frontend (Next.js static export)...");
        
        let status = Command::new("bun")
            .current_dir(&yeollin_app_dir)
            .args(["run", "next", "build"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .context("Failed to run next build")?;

        if !status.success() {
            anyhow::bail!("Next.js build failed");
        }

        info!("Frontend build complete: {}", yeollin_app_dir.join("out").display());
    }

    // 3. Build Rust binary
    if !args.skip_backend {
        info!("Building backend (Rust)...");

        let mut cargo_args = vec!["build"];
        if args.release {
            cargo_args.push("--release");
        }

        let status = Command::new("cargo")
            .current_dir(&project_dir)
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

    info!("Build complete!");
    Ok(())
}
