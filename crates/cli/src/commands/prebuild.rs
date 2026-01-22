//! Prebuild command
//!
//! Extracts the app template and links plugin frontend assets.

use std::path::{Path, PathBuf};
use std::fs;
use clap::Args;
use anyhow::{Context, Result};
use tracing::{info, debug};

use crate::template::AppTemplate;

#[derive(Args)]
pub struct PrebuildArgs {
    /// Project root directory (auto-detected if not specified)
    #[arg(short, long)]
    pub project_dir: Option<PathBuf>,

    /// Output directory for the generated app (default: .yeollin/app)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Force overwrite existing output
    #[arg(short, long)]
    pub force: bool,
}

/// Plugin frontend info discovered from filesystem
#[derive(Debug)]
pub struct PluginFrontend {
    pub name: String,
    pub plugin_path: PathBuf,  // Plugin root (plugins/<name>/)
    pub app_path: PathBuf,     // Frontend assets (plugins/<name>/app/)
}

/// Find project root by walking up directories
/// Looks for: plugins/ directory or workspace Cargo.toml
pub fn find_project_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    
    loop {
        // Check for plugins/ directory (primary indicator)
        if current.join("plugins").is_dir() {
            return Some(current);
        }
        
        // Check for workspace Cargo.toml
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Some(current);
                }
            }
        }
        
        // Move up one directory
        if !current.pop() {
            return None;
        }
    }
}

/// Detect if current directory is inside a plugin directory
/// Returns the plugin directory path if found
pub fn find_current_plugin_dir() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;
    let project_root = find_project_root()?;
    let plugins_dir = project_root.join("plugins");
    
    // Check if current dir is under plugins/
    if current.starts_with(&plugins_dir) {
        // Get the plugin directory (first level under plugins/)
        let relative = current.strip_prefix(&plugins_dir).ok()?;
        let plugin_name = relative.components().next()?;
        return Some(plugins_dir.join(plugin_name));
    }
    
    None
}

pub async fn run(args: PrebuildArgs) -> Result<()> {
    let project_dir = match args.project_dir {
        Some(dir) => dir,
        None => find_project_root()
            .context("Could not find project root. Run from project directory or use --project-dir")?,
    };
    
    let output_dir = args.output_dir
        .unwrap_or_else(|| project_dir.join(".yeollin").join("app"));

    info!("Prebuild starting...");
    info!("  Project: {}", project_dir.display());
    info!("  Output:  {}", output_dir.display());

    // 1. Discover plugins with frontend assets
    let plugins = discover_plugins(&project_dir)?;
    info!("Found {} plugins with frontend assets", plugins.len());

    run_with_plugins(&output_dir, &plugins, args.force).await
}

/// Run prebuild with an explicit list of plugins
pub async fn run_with_plugins(output_dir: &Path, plugins: &[PluginFrontend], force: bool) -> Result<()> {
    for plugin in plugins {
        debug!("  - {} at {}", plugin.name, plugin.app_path.display());
    }

    // 1. Extract or prepare app template
    prepare_output_dir(output_dir, force)?;
    
    // 2. Extract embedded template
    AppTemplate::extract_to(output_dir)?;
    info!("Extracted app template to {}", output_dir.display());

    // 3. Merge plugin dependencies into output package.json
    merge_plugin_dependencies(output_dir, plugins)?;
    info!("Merged plugin dependencies");

    // 4. Link plugin frontends
    link_plugins(output_dir, plugins)?;
    info!("Linked {} plugin frontends", plugins.len());

    // 5. Generate plugin manifest for Next.js
    generate_plugin_manifest(output_dir, plugins)?;
    info!("Generated plugin manifest");

    info!("Prebuild complete!");
    Ok(())
}

/// Discover plugins that have frontend assets (app/ directory)
/// 
/// Skips directories that look like complete Next.js apps (have next.config.*)
fn discover_plugins(project_dir: &Path) -> Result<Vec<PluginFrontend>> {
    let plugins_dir = project_dir.join("plugins");
    
    if !plugins_dir.exists() {
        return Ok(vec![]);
    }

    let mut plugins = vec![];

    for entry in fs::read_dir(&plugins_dir)
        .with_context(|| format!("Failed to read plugins directory: {}", plugins_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() {
            continue;
        }

        let plugin_name = path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let app_path = path.join("app");
        
        if app_path.exists() && app_path.is_dir() {
            // Skip if this looks like a complete Next.js app (not just plugin components)
            if is_complete_nextjs_app(&app_path) {
                debug!("Skipping {} - appears to be a complete Next.js app", plugin_name);
                continue;
            }
            
            plugins.push(PluginFrontend {
                name: plugin_name,
                plugin_path: path,
                app_path,
            });
        }
    }

    Ok(plugins)
}

/// Check if a directory is a complete Next.js app (has next.config.* file)
fn is_complete_nextjs_app(path: &Path) -> bool {
    let config_files = ["next.config.ts", "next.config.js", "next.config.mjs"];
    config_files.iter().any(|f| path.join(f).exists())
}

/// Prepare output directory
fn prepare_output_dir(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        if force {
            fs::remove_dir_all(output_dir)
                .with_context(|| format!("Failed to remove existing output: {}", output_dir.display()))?;
        } else {
            // Clean only the plugins symlink directory, keep rest
            let plugins_link_dir = output_dir.join("src").join("app").join("(plugins)");
            if plugins_link_dir.exists() {
                fs::remove_dir_all(&plugins_link_dir)?;
            }
        }
    }
    
    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
    
    Ok(())
}

/// Merge plugin dependencies into the output package.json
fn merge_plugin_dependencies(output_dir: &Path, plugins: &[PluginFrontend]) -> Result<()> {
    let package_json_path = output_dir.join("package.json");
    
    if !package_json_path.exists() {
        return Ok(());
    }

    // Read base package.json
    let content = fs::read_to_string(&package_json_path)?;
    let mut package: serde_json::Value = serde_json::from_str(&content)?;

    // Collect dependencies from plugins
    for plugin in plugins {
        let plugin_package_path = plugin.plugin_path.join("package.json");
        
        if !plugin_package_path.exists() {
            continue;
        }

        let plugin_content = fs::read_to_string(&plugin_package_path)?;
        let plugin_package: serde_json::Value = match serde_json::from_str(&plugin_content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Merge dependencies
        if let Some(deps) = plugin_package.get("dependencies").and_then(|d| d.as_object()) {
            let target_deps = package
                .get_mut("dependencies")
                .and_then(|d| d.as_object_mut());
            
            if let Some(target) = target_deps {
                for (key, value) in deps {
                    if !target.contains_key(key) {
                        debug!("Adding dependency from {}: {} = {}", plugin.name, key, value);
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        // Merge devDependencies
        if let Some(deps) = plugin_package.get("devDependencies").and_then(|d| d.as_object()) {
            let target_deps = package
                .get_mut("devDependencies")
                .and_then(|d| d.as_object_mut());
            
            if let Some(target) = target_deps {
                for (key, value) in deps {
                    if !target.contains_key(key) {
                        debug!("Adding devDependency from {}: {} = {}", plugin.name, key, value);
                        target.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    // Write merged package.json
    let merged = serde_json::to_string_pretty(&package)?;
    fs::write(&package_json_path, merged)?;

    Ok(())
}

/// Link plugin frontend directories into the app
fn link_plugins(output_dir: &Path, plugins: &[PluginFrontend]) -> Result<()> {
    // Plugins go under src/app/(plugins)/ for Next.js App Router
    let plugins_app_dir = output_dir.join("src").join("app").join("(plugins)");
    fs::create_dir_all(&plugins_app_dir)?;

    for plugin in plugins {
        let link_path = plugins_app_dir.join(&plugin.name);
        
        // Create symlink (or copy on Windows if symlink fails)
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&plugin.app_path, &link_path)
                .with_context(|| format!("Failed to symlink plugin: {}", plugin.name))?;
        }
        
        #[cfg(windows)]
        {
            // Try symlink first, fall back to junction, then copy
            if std::os::windows::fs::symlink_dir(&plugin.app_path, &link_path).is_err() {
                // Use junction as fallback (doesn't require admin)
                let status = std::process::Command::new("cmd")
                    .args(["/C", "mklink", "/J", 
                        &link_path.to_string_lossy(), 
                        &plugin.app_path.to_string_lossy()])
                    .status();
                
                if status.is_err() || !status.unwrap().success() {
                    // Last resort: copy
                    copy_dir_recursive(&plugin.app_path, &link_path)?;
                }
            }
        }

        debug!("Linked plugin '{}' to {}", plugin.name, link_path.display());
    }

    Ok(())
}

/// Generate plugin manifest JSON for Next.js to consume
fn generate_plugin_manifest(output_dir: &Path, plugins: &[PluginFrontend]) -> Result<()> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct PluginManifest {
        plugins: Vec<PluginEntry>,
    }

    #[derive(Serialize)]
    struct PluginEntry {
        name: String,
        route_prefix: String,
    }

    let manifest = PluginManifest {
        plugins: plugins.iter().map(|p| PluginEntry {
            name: p.name.clone(),
            route_prefix: format!("/(plugins)/{}", p.name),
        }).collect(),
    };

    let manifest_path = output_dir.join("plugins.json");
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, json)?;

    Ok(())
}

/// Recursively copy a directory (fallback for Windows)
#[cfg(windows)]
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
