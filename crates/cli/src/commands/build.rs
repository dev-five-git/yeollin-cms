//! Build command
//!
//! Runs prebuild, then builds Next.js for static export and Rust binary.

use std::path::PathBuf;
use std::process::Stdio;
use clap::Args;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, debug};
use serde::Deserialize;

use super::prebuild::{find_project_root, find_current_plugin_dir, PluginFrontend, PluginInfo, run_with_plugins_mode};

/// Plugin info from binary export
#[derive(Debug, Deserialize)]
struct ExportedPlugin {
    name: String,
    version: String,
    author: Option<String>,
    description: Option<String>,
    license: Option<String>,
    frontend_path: Option<String>,
}

/// Export plugin list by running the binary with YEOLLIN_EXPORT_PLUGINS=1
/// Returns (plugins for frontend linking, full plugin info for plugins.json)
async fn export_plugins_from_binary(
    project_dir: &PathBuf,
    api_dir: Option<&PathBuf>,
) -> Result<(Vec<PluginFrontend>, Vec<PluginInfo>)> {
    // Determine binary path
    let binary_name = if let Some(api) = api_dir {
        // Get crate name from Cargo.toml
        let cargo_toml = api.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;
            // Simple parse for name = "..."
            content.lines()
                .find(|l| l.trim().starts_with("name"))
                .and_then(|l| l.split('"').nth(1))
                .map(|s| s.to_string())
                .unwrap_or_else(|| "yeollin-app".to_string())
        } else {
            "yeollin-app".to_string()
        }
    } else {
        "yeollin-app".to_string()
    };

    let binary_path = project_dir.join("target").join("debug").join(format!("{}.exe", binary_name));
    
    // Handle non-Windows
    #[cfg(not(windows))]
    let binary_path = project_dir.join("target").join("debug").join(&binary_name);

    if !binary_path.exists() {
        anyhow::bail!("Binary not found: {}", binary_path.display());
    }

    debug!("Running {} with YEOLLIN_EXPORT_PLUGINS=1", binary_path.display());

    // Run binary with export flag
    let output = Command::new(&binary_path)
        .env("YEOLLIN_EXPORT_PLUGINS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .context("Failed to run binary for plugin export")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Find JSON array in output (skip any log lines)
    let json_start = stdout.rfind("\n[").map(|i| i + 1)
        .or_else(|| if stdout.starts_with('[') { Some(0) } else { None })
        .context("No JSON array in output")?;
    let json_end = stdout.rfind(']').context("No JSON array end in output")? + 1;
    let json_str = &stdout[json_start..json_end];
    
    let exported: Vec<ExportedPlugin> = serde_json::from_str(json_str)
        .context("Failed to parse plugin JSON")?;

    // Convert to PluginFrontend (for frontend linking)
    let frontend_plugins: Vec<PluginFrontend> = exported
        .iter()
        .filter_map(|p| {
            let frontend_path = p.frontend_path.as_ref()?;
            let app_path = PathBuf::from(frontend_path).canonicalize().ok()?;
            let plugin_path = app_path.parent()?.to_path_buf();
            
            Some(PluginFrontend {
                name: p.name.clone(),
                plugin_path,
                app_path,
            })
        })
        .collect();

    // Convert to PluginInfo (for plugins.json)
    let all_plugins: Vec<PluginInfo> = exported
        .into_iter()
        .map(|p| PluginInfo {
            name: p.name,
            version: p.version,
            author: p.author,
            description: p.description,
            license: p.license,
            frontend_path: p.frontend_path,
        })
        .collect();

    Ok((frontend_plugins, all_plugins))
}

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
    let project_dir = match args.project_dir.clone() {
        Some(dir) => dir,
        None => find_project_root()
            .context("Could not find project root. Run from project directory or use --project-dir")?,
    };

    // Determine plugin directory and API to run
    let plugin_dir = find_current_plugin_dir();
    let api_dir = plugin_dir.as_ref().map(|p| p.join("api"));
    
    // Determine output directory
    let output_base = plugin_dir.clone().unwrap_or_else(|| project_dir.clone());
    let yeollin_app_dir = output_base.join(".yeollin").join("app");

    // 1. Build API binary first (debug mode for plugin export)
    info!("Building API...");
    let build_status = if let Some(ref api) = api_dir {
        if api.exists() {
            Command::new("cargo")
                .current_dir(api)
                .args(["build"])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await?
        } else {
            Command::new("cargo")
                .current_dir(&project_dir)
                .args(["build", "-p", "yeollin-app"])
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await?
        }
    } else {
        Command::new("cargo")
            .current_dir(&project_dir)
            .args(["build", "-p", "yeollin-app"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await?
    };

    if !build_status.success() {
        anyhow::bail!("Failed to build API");
    }

    // 2. Export plugins from binary
    let (frontend_plugins, all_plugins) = if !args.skip_prebuild {
        info!("Discovering plugins from binary...");
        let (frontend_plugins, all_plugins) = export_plugins_from_binary(&project_dir, api_dir.as_ref()).await?;
        info!("Found {} registered plugins", all_plugins.len());
        (frontend_plugins, all_plugins)
    } else {
        (vec![], vec![])
    };

    // 3. Run prebuild if not skipped (use copy mode for production builds)
    if !args.skip_prebuild {
        info!("Running prebuild (copy mode for production)...");
        run_with_plugins_mode(&yeollin_app_dir, &frontend_plugins, &all_plugins, true, true).await?;
    }

    // 4. Install dependencies
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

    // 5. Build Next.js for static export (SSG)
    if !args.skip_frontend {
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

        info!("Frontend build complete: {}", yeollin_app_dir.join("out").display());
    }

    // 6. Build Rust binary in release mode with embedded static files
    if !args.skip_backend {
        info!("Building backend (Rust release with embedded static files)...");

        // Release mode auto-embeds static files via #[cfg(not(debug_assertions))]
        let mut cargo_args = vec!["build"];
        if args.release {
            cargo_args.push("--release");
        }

        // Build the specific API if inside a plugin
        if let Some(ref api) = api_dir {
            if api.exists() {
                let status = Command::new("cargo")
                    .current_dir(api)
                    .args(&cargo_args)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()
                    .await
                    .context("Failed to run cargo build")?;

                if !status.success() {
                    anyhow::bail!("Cargo build failed");
                }
            }
        } else {
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
        }

        let profile = if args.release { "release" } else { "debug" };
        info!("Backend build complete: target/{}/", profile);
    }

    info!("Build complete!");
    Ok(())
}
