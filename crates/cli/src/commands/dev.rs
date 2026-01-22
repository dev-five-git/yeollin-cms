//! Dev command
//!
//! Runs prebuild, then starts Next.js dev server and cargo watch.

use std::path::PathBuf;
use std::process::Stdio;
use clap::Args;
use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{info, debug};
use serde::Deserialize;

use super::prebuild::{self, find_project_root, find_current_plugin_dir, PluginFrontend};

/// Plugin info from binary export
#[derive(Debug, Deserialize)]
struct ExportedPlugin {
    name: String,
    #[allow(dead_code)]
    version: String,
    frontend_path: Option<String>,
}

/// Export plugin list by running the binary with YEOLLIN_EXPORT_PLUGINS=1
async fn export_plugins_from_binary(
    project_dir: &PathBuf,
    api_dir: Option<&PathBuf>,
) -> Result<Vec<PluginFrontend>> {
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
    // Look for '[' followed by newline or '{' to find actual JSON array start
    let json_start = stdout.rfind("\n[").map(|i| i + 1)
        .or_else(|| if stdout.starts_with('[') { Some(0) } else { None })
        .context("No JSON array in output")?;
    let json_end = stdout.rfind(']').context("No JSON array end in output")? + 1;
    let json_str = &stdout[json_start..json_end];
    
    let exported: Vec<ExportedPlugin> = serde_json::from_str(json_str)
        .context("Failed to parse plugin JSON")?;

    // Convert to PluginFrontend
    let plugins: Vec<PluginFrontend> = exported
        .into_iter()
        .filter_map(|p| {
            let frontend_path = p.frontend_path?;
            let app_path = PathBuf::from(&frontend_path).canonicalize().ok()?;
            let plugin_path = app_path.parent()?.to_path_buf();
            
            Some(PluginFrontend {
                name: p.name,
                plugin_path,
                app_path,
            })
        })
        .collect();

    Ok(plugins)
}

#[derive(Args)]
pub struct DevArgs {
    /// Project root directory (auto-detected if not specified)
    #[arg(short, long)]
    pub project_dir: Option<PathBuf>,

    /// Skip prebuild step
    #[arg(long)]
    pub skip_prebuild: bool,

    /// Port for Next.js dev server
    #[arg(long, default_value = "3000")]
    pub frontend_port: u16,

    /// Skip API server (frontend only)
    #[arg(long)]
    pub no_api: bool,

    /// Port for API server
    #[arg(long, default_value = "3001")]
    pub api_port: u16,
}

pub async fn run(args: DevArgs) -> Result<()> {
    let project_dir = match args.project_dir.clone() {
        Some(dir) => dir,
        None => find_project_root()
            .context("Could not find project root. Run from project directory or use --project-dir")?,
    };

    // Determine plugin directory and API to run
    let plugin_dir = find_current_plugin_dir();
    let api_dir = plugin_dir.as_ref().map(|p| p.join("api"));
    
    // Determine output directory - use plugin dir if inside one, otherwise project root
    let output_base = plugin_dir.clone().unwrap_or_else(|| project_dir.clone());
    let yeollin_app_dir = output_base.join(".yeollin").join("app");

    // 1. Build API binary first
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
    let plugins = if !args.skip_prebuild {
        info!("Discovering plugins from binary...");
        let plugins = export_plugins_from_binary(&project_dir, api_dir.as_ref()).await?;
        info!("Found {} registered plugins", plugins.len());
        plugins
    } else {
        vec![]
    };

    // 3. Run prebuild if not skipped
    if !args.skip_prebuild {
        info!("Running prebuild...");
        prebuild::run_with_plugins(&yeollin_app_dir, &plugins, false).await?;
    }

    // 2. Install dependencies if needed
    if !yeollin_app_dir.join("node_modules").exists() {
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

    // Determine which API to run
    let api_info = if let Some(ref plugin) = plugin_dir {
        let plugin_name = plugin.file_name().and_then(|n| n.to_str()).unwrap_or("plugin");
        format!("{}/api", plugin_name)
    } else {
        "yeollin-app".to_string()
    };

    info!("Starting development servers...");
    info!("  Frontend: http://localhost:{}", args.frontend_port);
    if !args.no_api {
        info!("  API:      http://localhost:{} ({})", args.api_port, api_info);
    }

    // 3. Start Next.js dev server
    let mut next_cmd = Command::new("bun");
    next_cmd
        .current_dir(&yeollin_app_dir)
        .args(["run", "dev"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let next_handle = next_cmd.spawn()?;

    // 4. Start API server (optional)
    let cargo_handle = if !args.no_api {
        let mut cargo_cmd = Command::new("cargo");
        
        // If inside a plugin, run plugin's API; otherwise run main app
        if let Some(ref plugin) = plugin_dir {
            let api_dir = plugin.join("api");
            if api_dir.exists() {
                cargo_cmd.current_dir(&api_dir).args(["run"]);
            } else {
                cargo_cmd.current_dir(&project_dir).args(["run", "-p", "yeollin-app"]);
            }
        } else {
            cargo_cmd.current_dir(&project_dir).args(["run", "-p", "yeollin-app"]);
        }
        
        cargo_cmd
            .env("PORT", args.api_port.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        Some(cargo_cmd.spawn()?)
    } else {
        None
    };

    info!("Development servers started. Press Ctrl+C to stop.");

    // Wait for processes
    match cargo_handle {
        Some(cargo) => {
            tokio::select! {
                result = next_handle.wait_with_output() => {
                    if let Err(e) = result {
                        tracing::error!("Next.js dev server error: {}", e);
                    }
                }
                result = cargo.wait_with_output() => {
                    if let Err(e) = result {
                        tracing::error!("API server error: {}", e);
                    }
                }
            }
        }
        None => {
            if let Err(e) = next_handle.wait_with_output().await {
                tracing::error!("Next.js dev server error: {}", e);
            }
        }
    }

    Ok(())
}
