//! Prebuild command
//!
//! Extracts the app template and links frontend assets from current directory.

use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use crate::template::AppTemplate;

#[derive(Args)]
pub struct PrebuildArgs {
    /// Output directory for the generated app (default: .yeollin/app)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Force overwrite existing output
    #[arg(short, long)]
    pub force: bool,
}

/// App info discovered from current directory
#[derive(Debug)]
pub struct AppFrontend {
    pub name: String,
    pub app_path: PathBuf, // Frontend assets (./app/)
}

/// Detect app structure in current directory
/// Returns AppFrontend if app/ directory exists
pub fn detect_current_app() -> Option<AppFrontend> {
    let current = std::env::current_dir().ok()?;
    let app_path = current.join("app");

    if app_path.exists() && app_path.is_dir() {
        let name = current
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(AppFrontend { name, app_path })
    } else {
        None
    }
}

/// Detect if current directory is a Rust crate
/// Supports both old structure (api/) and new structure (Cargo.toml at root)
pub fn detect_crate_dir() -> Option<PathBuf> {
    let current = std::env::current_dir().ok()?;

    // New structure: Cargo.toml at root with src/
    if current.join("Cargo.toml").exists() && current.join("src").is_dir() {
        return Some(current);
    }

    // Old structure: api/ subdirectory
    let api_dir = current.join("api");
    if api_dir.is_dir() && api_dir.join("Cargo.toml").exists() {
        return Some(api_dir);
    }

    None
}

/// Get binary name from Cargo.toml
pub fn get_binary_name(crate_dir: &Path) -> Result<String> {
    let cargo_toml = crate_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        let content = fs::read_to_string(&cargo_toml)?;
        // Look for [[bin]] name first
        if let Some(bin_name) = content
            .lines()
            .skip_while(|l| !l.contains("[[bin]]"))
            .find(|l| l.trim().starts_with("name"))
            .and_then(|l| l.split('"').nth(1))
        {
            return Ok(bin_name.to_string());
        }
        // Fall back to package name
        if let Some(pkg_name) = content
            .lines()
            .find(|l| l.trim().starts_with("name"))
            .and_then(|l| l.split('"').nth(1))
        {
            return Ok(pkg_name.to_string());
        }
    }
    Ok("yeollin-app".to_string())
}

/// Find binary path for the crate
pub fn find_binary_path(crate_dir: &Path) -> Result<std::path::PathBuf> {
    let binary_name = get_binary_name(crate_dir)?;

    // Find workspace root to locate target/debug
    let mut workspace_root = crate_dir.to_path_buf();
    while !workspace_root.join("target").exists() {
        if !workspace_root.pop() {
            anyhow::bail!("Could not find workspace root with target directory");
        }
    }

    #[cfg(windows)]
    let binary_path = workspace_root
        .join("target")
        .join("debug")
        .join(format!("{}.exe", binary_name));
    #[cfg(not(windows))]
    let binary_path = workspace_root
        .join("target")
        .join("debug")
        .join(&binary_name);

    if !binary_path.exists() {
        anyhow::bail!("Binary not found: {}", binary_path.display());
    }

    Ok(binary_path)
}

/// Export data from the built binary using an environment variable
pub async fn export_from_binary(binary_path: &Path, env_var: &str) -> Result<String> {
    debug!("Exporting {} from {}", env_var, binary_path.display());

    let output = Command::new(binary_path)
        .env(env_var, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .context(format!("Failed to run binary for {} export", env_var))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find JSON array in output
    let json_start = stdout
        .rfind("\n[")
        .map(|i| i + 1)
        .or_else(|| {
            if stdout.starts_with('[') {
                Some(0)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let json_str = &stdout[json_start..];

    Ok(json_str.trim().to_string())
}

pub async fn run(args: PrebuildArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| current_dir.join(".yeollin").join("app"));

    // Detect Rust crate (new structure: Cargo.toml at root, or old: api/)
    let crate_dir = detect_crate_dir();

    info!("Prebuild starting...");
    info!("  Current dir: {}", current_dir.display());
    info!("  Output:      {}", output_dir.display());
    if let Some(ref dir) = crate_dir {
        info!("  Crate:       {}", dir.display());
    }

    // Detect app/ in current directory
    let frontend = detect_current_app();

    if let Some(ref app) = frontend {
        info!("Found frontend: {} at {}", app.name, app.app_path.display());
    } else {
        info!("No app/ directory found, creating template only");
    }

    // Build crate first if exists
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

    // Export menus and plugins from binary
    let (menus_json, plugins_json) = if let Some(ref crate_path) = crate_dir {
        let binary_path = find_binary_path(crate_path)?;

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

    run_prebuild(
        &output_dir,
        frontend.as_ref(),
        menus_json.as_deref(),
        plugins_json.as_deref(),
        args.force,
    )
    .await
}

/// Run prebuild with optional frontend, menus, and plugins
pub async fn run_prebuild(
    output_dir: &Path,
    frontend: Option<&AppFrontend>,
    menus_json: Option<&str>,
    plugins_json: Option<&str>,
    force: bool,
) -> Result<()> {
    // 1. Prepare output directory
    prepare_output_dir(output_dir, force)?;

    // 2. Create .gitignore in .yeollin/ directory
    if let Some(yeollin_dir) = output_dir.parent() {
        let gitignore_path = yeollin_dir.join(".gitignore");
        fs::write(&gitignore_path, "*\n")?;
    }

    // 3. Extract embedded template
    AppTemplate::extract_to(output_dir)?;
    info!("Extracted app template to {}", output_dir.display());

    // 4. Copy openapi.json from api/ if it exists
    let current_dir = std::env::current_dir()?;
    copy_openapi_json(&current_dir, output_dir)?;

    // 5. If frontend exists, merge dependencies and link
    if let Some(app) = frontend {
        merge_dependencies(output_dir, &current_dir)?;
        info!("Merged dependencies");

        link_frontend(output_dir, app, true)?; // Always copy for now (Turbopack compatibility)
        info!("Linked frontend");
    }

    // 6. Write menus.json and plugins.json (from binary export or empty)
    write_menus(output_dir, menus_json)?;
    write_plugins(output_dir, plugins_json)?;

    // 7. Copy plugin frontend files (goes under (auth)/)
    let has_plugins = copy_plugin_frontends(output_dir, plugins_json)?;

    // 8. Ensure (auth)/layout.tsx exists if plugins were copied
    // Template should already have it, but generate if missing
    if has_plugins {
        let auth_dir = output_dir.join("src").join("app").join("(auth)");
        let auth_layout = auth_dir.join("layout.tsx");
        if !auth_layout.exists() {
            generate_auth_layout(&auth_dir)?;
            info!("Generated (auth)/layout.tsx for plugins");
        }
    }

    info!("Prebuild complete!");
    Ok(())
}

/// Copy openapi.json from api/ directory to output if it exists
fn copy_openapi_json(current_dir: &Path, output_dir: &Path) -> Result<()> {
    let api_openapi = current_dir.join("api").join("openapi.json");

    if api_openapi.exists() {
        let dest = output_dir.join("openapi.json");
        fs::copy(&api_openapi, &dest)?;
        info!("Copied openapi.json from api/");
    } else {
        // Create empty openapi.json placeholder so Next.js config doesn't fail
        let placeholder = serde_json::json!({
            "openapi": "3.1.0",
            "info": {
                "title": "API",
                "version": "0.1.0"
            },
            "paths": {}
        });
        let dest = output_dir.join("openapi.json");
        fs::write(&dest, serde_json::to_string_pretty(&placeholder)?)?;
        debug!("Created placeholder openapi.json");
    }

    Ok(())
}

/// Prepare output directory
fn prepare_output_dir(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        if force {
            fs::remove_dir_all(output_dir).with_context(|| {
                format!("Failed to remove existing output: {}", output_dir.display())
            })?;
        } else {
            // Clean route group directories, keep rest
            let app_base = output_dir.join("src").join("app");
            for dir_name in ["(public)", "(auth)", "(app)"] {
                let dir = app_base.join(dir_name);
                if dir.exists() {
                    fs::remove_dir_all(&dir)?;
                }
            }
        }
    }

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    Ok(())
}

/// Merge dependencies from current directory's package.json into output
fn merge_dependencies(output_dir: &Path, app_dir: &Path) -> Result<()> {
    let output_package = output_dir.join("package.json");
    let app_package = app_dir.join("package.json");

    if !output_package.exists() || !app_package.exists() {
        return Ok(());
    }

    let output_content = fs::read_to_string(&output_package)?;
    let mut output_json: serde_json::Value = serde_json::from_str(&output_content)?;

    let app_content = fs::read_to_string(&app_package)?;
    let app_json: serde_json::Value = serde_json::from_str(&app_content)?;

    // Merge dependencies
    if let Some(deps) = app_json.get("dependencies").and_then(|d| d.as_object()) {
        if let Some(target) = output_json
            .get_mut("dependencies")
            .and_then(|d| d.as_object_mut())
        {
            for (key, value) in deps {
                if !target.contains_key(key) {
                    debug!("Adding dependency: {} = {}", key, value);
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }

    // Merge devDependencies
    if let Some(deps) = app_json.get("devDependencies").and_then(|d| d.as_object()) {
        if let Some(target) = output_json
            .get_mut("devDependencies")
            .and_then(|d| d.as_object_mut())
        {
            for (key, value) in deps {
                if !target.contains_key(key) {
                    debug!("Adding devDependency: {} = {}", key, value);
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }

    let merged = serde_json::to_string_pretty(&output_json)?;
    fs::write(&output_package, merged)?;

    Ok(())
}

/// Link or copy frontend directory into the app
/// Separates routes into (public)/ and (auth)/ based on (public) marker in source path
/// - Routes with (public) anywhere in path → src/app/(public)/...
/// - Routes without (public) → src/app/(auth)/...
fn link_frontend(output_dir: &Path, frontend: &AppFrontend, _copy_mode: bool) -> Result<()> {
    use walkdir::WalkDir;

    let public_dir = output_dir.join("src").join("app").join("(public)");
    let auth_dir = output_dir.join("src").join("app").join("(auth)");

    let mut has_public_routes = false;
    let mut has_auth_routes = false;

    // Walk all files in frontend directory
    for entry in WalkDir::new(&frontend.app_path) {
        let entry = entry?;
        let src_path = entry.path();

        // Skip directories, we only care about files
        if src_path.is_dir() {
            continue;
        }

        // Get relative path from app_path
        let rel_path = src_path
            .strip_prefix(&frontend.app_path)
            .unwrap_or(src_path);
        let rel_str = rel_path.to_string_lossy();

        // Check if (public) appears anywhere in the path
        let is_public = rel_str.contains("(public)");

        // Build clean path by stripping all route groups like (xxx)
        let clean_path = strip_route_groups(rel_path);

        // Determine destination
        let dest_path = if is_public {
            has_public_routes = true;
            public_dir.join(&clean_path)
        } else {
            has_auth_routes = true;
            auth_dir.join(&clean_path)
        };

        // Create parent directories and copy file
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src_path, &dest_path)?;
        debug!(
            "Copied {} -> {} ({})",
            rel_str,
            dest_path.display(),
            if is_public { "public" } else { "auth" }
        );
    }

    // Generate layout files only if they don't already exist (template may have them)
    if has_public_routes {
        let public_layout = public_dir.join("layout.tsx");
        if !public_layout.exists() {
            generate_public_layout(&public_dir)?;
            info!("Generated (public)/layout.tsx");
        }
    }
    if has_auth_routes {
        let auth_layout = auth_dir.join("layout.tsx");
        if !auth_layout.exists() {
            generate_auth_layout(&auth_dir)?;
            info!("Generated (auth)/layout.tsx");
        }
    }

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

/// Generate a minimal layout for public routes (no auth required)
fn generate_public_layout(public_dir: &Path) -> Result<()> {
    let layout_path = public_dir.join("layout.tsx");
    let content = r#"export default function PublicLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}
"#;
    fs::create_dir_all(public_dir)?;
    fs::write(&layout_path, content)?;
    Ok(())
}

/// Generate a minimal layout for authenticated routes
fn generate_auth_layout(auth_dir: &Path) -> Result<()> {
    let layout_path = auth_dir.join("layout.tsx");
    let content = r#"export default function AuthLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // TODO: Add authentication check here
  return <>{children}</>;
}
"#;
    fs::create_dir_all(auth_dir)?;
    fs::write(&layout_path, content)?;
    Ok(())
}

/// Write menus.json from exported menus or empty array
/// Transforms MenuConfig[] to MenuItem[] for frontend consumption
fn write_menus(output_dir: &Path, menus_json: Option<&str>) -> Result<()> {
    let menus_path = output_dir.join("src").join("menus.json");

    let content = if let Some(json_str) = menus_json {
        // Parse the MenuConfig[] from binary export
        let menu_configs: Vec<serde_json::Value> =
            serde_json::from_str(json_str).unwrap_or_default();

        // Flatten: extract all items from each MenuConfig
        let menu_items: Vec<serde_json::Value> = menu_configs
            .into_iter()
            .filter_map(|config| config.get("items").cloned())
            .filter_map(|items| items.as_array().cloned())
            .flatten()
            .collect();

        serde_json::to_string_pretty(&menu_items).unwrap_or_else(|_| "[]".to_string())
    } else {
        "[]".to_string()
    };

    fs::write(&menus_path, content)?;
    info!("Wrote menus.json");
    Ok(())
}

/// Write plugins.json from exported plugins or empty array
fn write_plugins(output_dir: &Path, plugins_json: Option<&str>) -> Result<()> {
    let plugins_path = output_dir.join("src").join("plugins.json");
    let content = plugins_json.unwrap_or("[]");
    fs::write(&plugins_path, content)?;
    info!("Wrote plugins.json");
    Ok(())
}

/// Copy plugin frontend files into the output directory
/// Parses plugins_json to get frontend_path and copies route groups
/// Plugins go under (auth)/<plugin-name>/ since they require authentication
/// Returns true if any plugins were copied
fn copy_plugin_frontends(output_dir: &Path, plugins_json: Option<&str>) -> Result<bool> {
    let Some(json_str) = plugins_json else {
        return Ok(false);
    };

    let plugins: Vec<serde_json::Value> = serde_json::from_str(json_str).unwrap_or_default();
    let mut copied_any = false;

    for plugin in plugins {
        let Some(name) = plugin.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(frontend_path) = plugin.get("frontend_path").and_then(|v| v.as_str()) else {
            continue;
        };

        let frontend_dir = Path::new(frontend_path);
        if !frontend_dir.exists() || !frontend_dir.is_dir() {
            debug!("Plugin {} frontend path not found: {}", name, frontend_path);
            continue;
        }

        // Destination: .yeollin/app/src/app/(auth)/<plugin-name>/
        // Plugins require authentication, so they go under (auth)/
        let dest_base = output_dir
            .join("src")
            .join("app")
            .join("(auth)")
            .join(name);

        // Scan for route groups (folders starting with parentheses) and copy their contents
        for entry in fs::read_dir(frontend_dir)? {
            let entry = entry?;
            let entry_path = entry.path();

            if !entry_path.is_dir() {
                continue;
            }

            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_str().unwrap_or("");

            // Check if it's a route group like (example)
            if dir_name_str.starts_with('(') && dir_name_str.ends_with(')') {
                // Copy contents of route group to plugin destination
                // e.g., (example)/items/* -> <plugin-name>/items/*
                copy_dir_contents(&entry_path, &dest_base)?;
                info!("Copied plugin frontend: {} from {}", name, dir_name_str);
                copied_any = true;
            }
        }
    }

    Ok(copied_any)
}

/// Copy contents of a directory to destination (not the directory itself)
/// Handles nested folders and files
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
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
