//! Prebuild command
//!
//! Extracts the app template and links frontend assets from current directory.

use anyhow::{Context, Result};
use async_walkdir::{DirEntry, WalkDir};
use clap::Args;
use futures_util::stream::{FuturesUnordered, StreamExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
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

    /// Use proxy mode (re-export from source instead of copying)
    /// This enables instant HMR in dev mode
    #[arg(long)]
    pub proxy: bool,
}

/// App info discovered from current directory
#[derive(Debug, Clone)]
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
pub async fn get_binary_name(crate_dir: &Path) -> Result<String> {
    let cargo_toml = crate_dir.join("Cargo.toml");
    if cargo_toml.exists() {
        let content = fs::read_to_string(&cargo_toml).await?;
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

/// Resolve the Cargo target directory for a crate.
///
/// Asks Cargo instead of assuming `<workspace>/target`, so that a
/// `CARGO_TARGET_DIR` env var or a `build.target-dir` override in any
/// `.cargo/config.toml` (user-global or project-local) is honored.
async fn find_target_dir(crate_dir: &Path) -> Result<PathBuf> {
    let output = Command::new("cargo")
        .current_dir(crate_dir)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .context("Failed to run `cargo metadata`")?;

    if !output.status.success() {
        anyhow::bail!(
            "`cargo metadata` failed in {}: {}",
            crate_dir.display(),
            output.status
        );
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse `cargo metadata` output")?;

    metadata
        .get("target_directory")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .context("`cargo metadata` did not report a target_directory")
}

/// Find binary path for the crate
pub async fn find_binary_path(crate_dir: &Path) -> Result<PathBuf> {
    let binary_name = get_binary_name(crate_dir).await?;
    let target_dir = find_target_dir(crate_dir).await?;

    let file_name = if cfg!(windows) {
        format!("{binary_name}.exe")
    } else {
        binary_name
    };
    let binary_path = target_dir.join("debug").join(file_name);

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
    info!(
        "  Mode:        {}",
        if args.proxy { "proxy" } else { "copy" }
    );
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

    // Export menus and plugins from binary IN PARALLEL
    let (menus_json, plugins_json) = if let Some(ref crate_path) = crate_dir {
        let binary_path = find_binary_path(crate_path).await?;

        info!("Exporting menus and plugins from binary (parallel)...");

        // Run both exports in parallel
        let (menus_result, plugins_result) = tokio::join!(
            export_from_binary(&binary_path, "YEOLLIN_EXPORT_MENUS"),
            export_from_binary(&binary_path, "YEOLLIN_EXPORT_PLUGINS")
        );

        let menus = match menus_result {
            Ok(m) => {
                info!("Exported menus successfully");
                Some(m)
            }
            Err(e) => {
                debug!("Could not export menus: {}", e);
                None
            }
        };

        let plugins = match plugins_result {
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
        args.proxy,
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
    use_proxy: bool,
) -> Result<()> {
    // 1. Prepare output directory
    prepare_output_dir(output_dir, force).await?;

    // 2. Create .gitignore in .yeollin/ directory
    if let Some(yeollin_dir) = output_dir.parent() {
        let gitignore_path = yeollin_dir.join(".gitignore");
        fs::write(&gitignore_path, "*\n").await?;
    }

    // 3. Extract embedded template
    AppTemplate::extract_to(output_dir).await?;
    info!("Extracted app template to {}", output_dir.display());

    // 4. Copy openapi.json from api/ if it exists
    let current_dir = std::env::current_dir()?;
    copy_openapi_json(&current_dir, output_dir).await?;

    // 5. If frontend exists, merge dependencies, link, and collect public routes
    if let Some(app) = frontend {
        merge_dependencies(output_dir, &current_dir).await?;
        info!("Merged dependencies");

        if use_proxy {
            link_frontend_proxy(output_dir, app).await?;
            info!("Linked frontend (proxy mode - instant HMR)");
        } else {
            link_frontend_copy(output_dir, app).await?;
            info!("Linked frontend (copy mode)");
        }
    }

    // 6. Write menus.json and plugins.json IN PARALLEL
    let menus_json_owned = menus_json.map(|s| s.to_string());
    let plugins_json_owned = plugins_json.map(|s| s.to_string());
    let output_dir_owned = output_dir.to_path_buf();
    let output_dir_owned2 = output_dir.to_path_buf();

    let (menus_result, plugins_result) = tokio::join!(
        write_menus(&output_dir_owned, menus_json_owned.as_deref()),
        write_plugins(&output_dir_owned2, plugins_json_owned.as_deref())
    );
    menus_result?;
    plugins_result?;

    // 7. Copy plugin frontend files (always copy, no proxy for plugins)
    let has_plugins = copy_plugin_frontends(output_dir, plugins_json).await?;

    // 8. Ensure (auth)/layout.tsx exists if plugins were copied
    if has_plugins {
        let auth_dir = output_dir.join("src").join("app").join("(auth)");
        let auth_layout = auth_dir.join("layout.tsx");
        if !auth_layout.exists() {
            generate_auth_layout(&auth_dir).await?;
            info!("Generated (auth)/layout.tsx for plugins");
        }
    }

    info!("Prebuild complete!");
    Ok(())
}

/// Copy openapi.json from api/ directory to output if it exists
async fn copy_openapi_json(current_dir: &Path, output_dir: &Path) -> Result<()> {
    let api_openapi = current_dir.join("api").join("openapi.json");

    if api_openapi.exists() {
        let dest = output_dir.join("openapi.json");
        fs::copy(&api_openapi, &dest).await?;
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
        fs::write(&dest, serde_json::to_string_pretty(&placeholder)?).await?;
        debug!("Created placeholder openapi.json");
    }

    Ok(())
}

/// Prepare output directory
async fn prepare_output_dir(output_dir: &Path, force: bool) -> Result<()> {
    if output_dir.exists() {
        if force {
            fs::remove_dir_all(output_dir).await.with_context(|| {
                format!("Failed to remove existing output: {}", output_dir.display())
            })?;
        } else {
            // Clean route group directories IN PARALLEL
            let app_base = output_dir.join("src").join("app");
            let dirs_to_remove: Vec<_> = ["(public)", "(auth)", "(app)"]
                .iter()
                .map(|name| app_base.join(name))
                .filter(|dir| dir.exists())
                .collect();

            let mut futures = FuturesUnordered::new();
            for dir in dirs_to_remove {
                futures.push(async move { fs::remove_dir_all(&dir).await });
            }
            while let Some(result) = futures.next().await {
                result?;
            }
        }
    }

    fs::create_dir_all(output_dir).await.with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    Ok(())
}

/// Merge dependencies from current directory's package.json into output
async fn merge_dependencies(output_dir: &Path, app_dir: &Path) -> Result<()> {
    let output_package = output_dir.join("package.json");
    let app_package = app_dir.join("package.json");

    if !output_package.exists() || !app_package.exists() {
        return Ok(());
    }

    // Read both files IN PARALLEL
    let (output_content, app_content) = tokio::join!(
        fs::read_to_string(&output_package),
        fs::read_to_string(&app_package)
    );
    let output_content = output_content?;
    let app_content = app_content?;

    let mut output_json: serde_json::Value = serde_json::from_str(&output_content)?;
    let app_json: serde_json::Value = serde_json::from_str(&app_content)?;

    // Collect all existing package names from both sections to avoid duplicates
    let existing_deps: std::collections::HashSet<String> = output_json
        .get("dependencies")
        .and_then(|d| d.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    let existing_dev_deps: std::collections::HashSet<String> = output_json
        .get("devDependencies")
        .and_then(|d| d.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    // Merge dependencies
    if let Some(deps) = app_json.get("dependencies").and_then(|d| d.as_object()) {
        if let Some(target) = output_json
            .get_mut("dependencies")
            .and_then(|d| d.as_object_mut())
        {
            for (key, value) in deps {
                if !existing_deps.contains(key) && !existing_dev_deps.contains(key) {
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
                if !existing_deps.contains(key) && !existing_dev_deps.contains(key) {
                    debug!("Adding devDependency: {} = {}", key, value);
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }

    let merged = serde_json::to_string_pretty(&output_json)?;
    fs::write(&output_package, merged).await?;

    Ok(())
}

/// Link frontend using PROXY mode - creates re-export files that import from source
/// This enables instant HMR since Next.js watches the original source files
async fn link_frontend_proxy(output_dir: &Path, frontend: &AppFrontend) -> Result<()> {
    let public_dir = output_dir.join("src").join("app").join("(public)");
    let auth_dir = output_dir.join("src").join("app").join("(auth)");

    let mut has_public_routes = false;
    let mut has_auth_routes = false;

    // Collect all files
    let mut files_to_proxy: Vec<(PathBuf, PathBuf, PathBuf, bool)> = Vec::new(); // (src, dest, rel_for_import, is_public)

    let mut walker = WalkDir::new(&frontend.app_path);
    while let Some(entry) = walker.next().await {
        let entry: DirEntry = entry?;
        let src_path = entry.path();

        if src_path.is_dir() {
            continue;
        }

        let rel_path = src_path
            .strip_prefix(&frontend.app_path)
            .unwrap_or(&src_path);
        let rel_str = rel_path.to_string_lossy();
        let is_public = rel_str.contains("(public)");
        let clean_path = strip_route_groups(rel_path);

        let dest_path = if is_public {
            has_public_routes = true;
            public_dir.join(&clean_path)
        } else {
            has_auth_routes = true;
            auth_dir.join(&clean_path)
        };

        files_to_proxy.push((
            src_path.to_path_buf(),
            dest_path,
            rel_path.to_path_buf(),
            is_public,
        ));
    }

    // Create all parent directories first
    let mut dirs_to_create: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for (_, dest_path, _, _) in &files_to_proxy {
        if let Some(parent) = dest_path.parent() {
            dirs_to_create.insert(parent.to_path_buf());
        }
    }
    for dir in dirs_to_create {
        fs::create_dir_all(&dir).await?;
    }

    // Create proxy files IN PARALLEL
    let mut futures = FuturesUnordered::new();

    for (src_path, dest_path, rel_path, is_public) in files_to_proxy {
        let future = async move {
            let extension = dest_path.extension().and_then(|e| e.to_str()).unwrap_or("");

            match extension {
                // TypeScript/JavaScript files - create re-export proxy
                "tsx" | "ts" | "jsx" | "js" => {
                    // Calculate relative path from dest to source
                    // dest: .yeollin/app/src/app/(public)/signin/page.tsx
                    // src:  app/(public)/signin/page.tsx
                    // We need: ../../../../../../app/(public)/signin/page

                    let dest_dir = dest_path.parent().unwrap();
                    let depth = dest_dir
                        .strip_prefix(std::env::current_dir().unwrap().join(".yeollin"))
                        .map(|p| p.components().count())
                        .unwrap_or(5);

                    let up_path = "../".repeat(depth + 1); // +1 for .yeollin itself

                    // Remove extension for import
                    let import_path = rel_path.with_extension("");
                    let import_path_str = import_path.to_string_lossy().replace('\\', "/");

                    let content = if extension == "tsx" || extension == "jsx" {
                        // For React components, re-export default and named exports
                        format!(
                            "export {{ default }} from \"{up_path}app/{import_path_str}\";\nexport * from \"{up_path}app/{import_path_str}\";\n"
                        )
                    } else {
                        // For plain TS/JS, just re-export everything
                        format!("export * from \"{up_path}app/{import_path_str}\";\n")
                    };

                    fs::write(&dest_path, content).await?;
                    debug!(
                        "Proxy {} -> {} ({})",
                        rel_path.display(),
                        dest_path.display(),
                        if is_public { "public" } else { "auth" }
                    );
                }
                // Non-JS files (CSS, images, etc.) - copy directly
                _ => {
                    fs::copy(&src_path, &dest_path).await?;
                    debug!(
                        "Copied {} -> {} ({})",
                        rel_path.display(),
                        dest_path.display(),
                        if is_public { "public" } else { "auth" }
                    );
                }
            }

            Ok::<_, anyhow::Error>(())
        };
        futures.push(future);
    }

    while let Some(result) = futures.next().await {
        result?;
    }

    // Generate layout files if needed
    if has_public_routes {
        let public_layout = public_dir.join("layout.tsx");
        if !public_layout.exists() {
            generate_public_layout(&public_dir).await?;
            info!("Generated (public)/layout.tsx");
        }
    }
    if has_auth_routes {
        let auth_layout = auth_dir.join("layout.tsx");
        if !auth_layout.exists() {
            generate_auth_layout(&auth_dir).await?;
            info!("Generated (auth)/layout.tsx");
        }
    }

    Ok(())
}

/// Link frontend using COPY mode - copies files directly
async fn link_frontend_copy(output_dir: &Path, frontend: &AppFrontend) -> Result<()> {
    let public_dir = output_dir.join("src").join("app").join("(public)");
    let auth_dir = output_dir.join("src").join("app").join("(auth)");

    let mut has_public_routes = false;
    let mut has_auth_routes = false;

    let mut files_to_copy: Vec<(PathBuf, PathBuf, bool)> = Vec::new();

    let mut walker = WalkDir::new(&frontend.app_path);
    while let Some(entry) = walker.next().await {
        let entry: DirEntry = entry?;
        let src_path = entry.path();

        if src_path.is_dir() {
            continue;
        }

        let rel_path = src_path
            .strip_prefix(&frontend.app_path)
            .unwrap_or(&src_path);
        let rel_str = rel_path.to_string_lossy();
        let is_public = rel_str.contains("(public)");
        let clean_path = strip_route_groups(rel_path);

        let dest_path = if is_public {
            has_public_routes = true;
            public_dir.join(&clean_path)
        } else {
            has_auth_routes = true;
            auth_dir.join(&clean_path)
        };

        files_to_copy.push((src_path.to_path_buf(), dest_path, is_public));
    }

    // Create directories
    let mut dirs_to_create: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for (_, dest_path, _) in &files_to_copy {
        if let Some(parent) = dest_path.parent() {
            dirs_to_create.insert(parent.to_path_buf());
        }
    }
    for dir in dirs_to_create {
        fs::create_dir_all(&dir).await?;
    }

    // Copy files IN PARALLEL
    const PARALLEL_COPIES: usize = 32;
    let mut futures = FuturesUnordered::new();

    for (src_path, dest_path, is_public) in files_to_copy {
        let future = async move {
            fs::copy(&src_path, &dest_path).await?;
            debug!(
                "Copied {} -> {} ({})",
                src_path.display(),
                dest_path.display(),
                if is_public { "public" } else { "auth" }
            );
            Ok::<_, anyhow::Error>(())
        };
        futures.push(future);

        if futures.len() >= PARALLEL_COPIES {
            if let Some(result) = futures.next().await {
                result?;
            }
        }
    }

    while let Some(result) = futures.next().await {
        result?;
    }

    // Generate layout files
    if has_public_routes {
        let public_layout = public_dir.join("layout.tsx");
        if !public_layout.exists() {
            generate_public_layout(&public_dir).await?;
            info!("Generated (public)/layout.tsx");
        }
    }
    if has_auth_routes {
        let auth_layout = auth_dir.join("layout.tsx");
        if !auth_layout.exists() {
            generate_auth_layout(&auth_dir).await?;
            info!("Generated (auth)/layout.tsx");
        }
    }

    Ok(())
}

/// Strip all route groups (parenthesized segments) from a path
fn strip_route_groups(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            if !(name_str.starts_with('(') && name_str.ends_with(')')) {
                result.push(name);
            }
        }
    }
    result
}

/// Generate a minimal layout for public routes (no auth required)
async fn generate_public_layout(public_dir: &Path) -> Result<()> {
    let layout_path = public_dir.join("layout.tsx");
    let content = r#"export default function PublicLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}
"#;
    fs::create_dir_all(public_dir).await?;
    fs::write(&layout_path, content).await?;
    Ok(())
}

/// Generate a minimal layout for authenticated routes
async fn generate_auth_layout(auth_dir: &Path) -> Result<()> {
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
    fs::create_dir_all(auth_dir).await?;
    fs::write(&layout_path, content).await?;
    Ok(())
}

/// Write menus.json from exported menus or empty array
async fn write_menus(output_dir: &Path, menus_json: Option<&str>) -> Result<()> {
    let menus_path = output_dir.join("src").join("menus.json");

    let content = if let Some(json_str) = menus_json {
        let menu_configs: Vec<serde_json::Value> =
            serde_json::from_str(json_str).unwrap_or_default();

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

    fs::write(&menus_path, content).await?;
    info!("Wrote menus.json");
    Ok(())
}

/// Write plugins.json from exported plugins or empty array
async fn write_plugins(output_dir: &Path, plugins_json: Option<&str>) -> Result<()> {
    let plugins_path = output_dir.join("src").join("plugins.json");
    let content = plugins_json.unwrap_or("[]");
    fs::write(&plugins_path, content).await?;
    info!("Wrote plugins.json");
    Ok(())
}

/// Copy plugin frontend files
async fn copy_plugin_frontends(output_dir: &Path, plugins_json: Option<&str>) -> Result<bool> {
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

        let dest_base = output_dir.join("src").join("app").join("(auth)").join(name);

        let mut entries = fs::read_dir(frontend_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();

            if !entry_path.is_dir() {
                continue;
            }

            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_str().unwrap_or("");

            if dir_name_str.starts_with('(') && dir_name_str.ends_with(')') {
                copy_dir_contents_parallel(&entry_path, &dest_base).await?;
                info!("Copied plugin frontend: {} from {}", name, dir_name_str);
                copied_any = true;
            }
        }
    }

    Ok(copied_any)
}

/// Copy contents of a directory to destination with parallel file copying
async fn copy_dir_contents_parallel(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).await?;

    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut dirs: Vec<(PathBuf, PathBuf)> = Vec::new();

    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            dirs.push((src_path, dst_path));
        } else {
            files.push((src_path, dst_path));
        }
    }

    // Copy files in parallel
    let mut futures = FuturesUnordered::new();
    for (src_path, dst_path) in files {
        futures.push(async move { fs::copy(&src_path, &dst_path).await });
    }
    while let Some(result) = futures.next().await {
        result?;
    }

    // Recurse into directories in parallel
    let mut dir_futures = FuturesUnordered::new();
    for (src_path, dst_path) in dirs {
        dir_futures.push(copy_dir_recursive_parallel(src_path, dst_path));
    }
    while let Some(result) = dir_futures.next().await {
        result?;
    }

    Ok(())
}

async fn copy_dir_recursive_parallel(src: PathBuf, dst: PathBuf) -> Result<()> {
    fs::create_dir_all(&dst).await?;

    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut dirs: Vec<(PathBuf, PathBuf)> = Vec::new();

    let mut entries = fs::read_dir(&src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            dirs.push((src_path, dst_path));
        } else {
            files.push((src_path, dst_path));
        }
    }

    let mut futures = FuturesUnordered::new();
    for (src_path, dst_path) in files {
        futures.push(async move { fs::copy(&src_path, &dst_path).await });
    }
    while let Some(result) = futures.next().await {
        result?;
    }

    let mut dir_futures = FuturesUnordered::new();
    for (src_path, dst_path) in dirs {
        dir_futures.push(copy_dir_recursive_parallel(src_path, dst_path));
    }
    while let Some(result) = dir_futures.next().await {
        result?;
    }

    Ok(())
}
