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
use yeollin_core::{ExportEnvelope, PluginSettingsInfo, EXPORT_ENV_VAR, EXPORT_SCHEMA_VERSION};

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

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("Failed to parse `cargo metadata` output")?;

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

/// Read the metadata envelope from a built binary.
///
/// The binary must emit exactly one JSON document on stdout and exit zero. Any
/// other outcome is an error: a partially-parsed manifest would silently drop
/// plugins, routes, or access rules from the assembled app.
pub async fn export_metadata(binary_path: &Path) -> Result<ExportEnvelope> {
    debug!("Exporting metadata from {}", binary_path.display());

    let output = Command::new(binary_path)
        .env(EXPORT_ENV_VAR, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| {
            format!(
                "Failed to run {} for metadata export",
                binary_path.display()
            )
        })?;

    if !output.status.success() {
        anyhow::bail!(
            "{} exited with {} during metadata export.\n{}",
            binary_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .context("metadata export produced non-UTF-8 output on stdout")?;

    parse_export_envelope(&stdout)
        .with_context(|| format!("reading metadata export from {}", binary_path.display()))
}

/// Parse the stdout of an export run into an envelope.
///
/// The whole stream must be one envelope. Recovering a payload from a partly
/// polluted stream is exactly what the previous "scan for a JSON array"
/// heuristic did, and it silently produced manifests missing plugins or routes.
pub(crate) fn parse_export_envelope(stdout: &str) -> Result<ExportEnvelope> {
    let trimmed = stdout.trim();

    if trimmed.is_empty() {
        anyhow::bail!("metadata export wrote nothing to stdout");
    }

    let envelope: ExportEnvelope = serde_json::from_str(trimmed).map_err(|error| {
        anyhow::anyhow!(
            "stdout is not a metadata envelope ({error}). \
             Application logs must go to stderr so stdout carries only the envelope."
        )
    })?;

    if envelope.schema_version != EXPORT_SCHEMA_VERSION {
        anyhow::bail!(
            "metadata envelope has schema version {}, but this CLI understands {}. \
             Rebuild the application against a matching yeollin version.",
            envelope.schema_version,
            EXPORT_SCHEMA_VERSION
        );
    }

    Ok(envelope)
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

    // A failed export is fatal: assembling the frontend from partial metadata
    // would silently drop plugins, routes, or access rules.
    let metadata = match crate_dir {
        Some(ref crate_path) => {
            let binary_path = find_binary_path(crate_path).await?;
            let envelope = export_metadata(&binary_path).await?;
            info!(
                plugins = envelope.plugins.len(),
                routes = envelope.routes.len(),
                "Exported metadata from binary"
            );
            Some(envelope)
        }
        None => None,
    };

    run_prebuild(
        &output_dir,
        &current_dir,
        frontend.as_ref(),
        metadata.as_ref(),
        args.force,
        args.proxy,
    )
    .await
}

/// Run prebuild with optional frontend and exported application metadata
pub async fn run_prebuild(
    output_dir: &Path,
    app_dir: &Path,
    frontend: Option<&AppFrontend>,
    metadata: Option<&ExportEnvelope>,
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
    copy_openapi_json(app_dir, output_dir).await?;

    // 5. If frontend exists, merge dependencies, link, and collect public routes
    if let Some(app) = frontend {
        merge_dependencies(output_dir, app_dir, metadata).await?;
        info!("Merged dependencies");

        if use_proxy {
            link_frontend_proxy(output_dir, app).await?;
            info!("Linked frontend (proxy mode - instant HMR)");
        } else {
            link_frontend_copy(output_dir, app).await?;
            info!("Linked frontend (copy mode)");
        }
    }

    // 6. Write the generated manifests
    write_manifests(output_dir, metadata).await?;

    // 7. Copy plugin frontend files (always copy, no proxy for plugins)
    let has_plugins = copy_plugin_frontends(output_dir, metadata).await?;

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
async fn copy_openapi_json(app_dir: &Path, output_dir: &Path) -> Result<()> {
    let api_openapi = app_dir.join("api").join("openapi.json");

    if api_openapi.exists() {
        let dest = output_dir.join("openapi.json");
        fs::copy(&api_openapi, &dest).await?;
        info!("Copied openapi.json from api/");
    } else {
        // Create empty openapi.json placeholder so the Devup API plugin can initialize
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
/// Merge the frontend dependencies of the host app and every plugin into the
/// generated app.
///
/// Only `dependencies` are merged. A `devDependencies` entry belongs to that
/// crate's own editor and typecheck setup; the assembled app gets its tooling
/// from the template.
///
/// A conflicting requirement is an error rather than a silent skip: the loser
/// would resolve against a version its pages were not written for, which is
/// exactly the failure this merge exists to prevent.
async fn merge_dependencies(
    output_dir: &Path,
    app_dir: &Path,
    metadata: Option<&ExportEnvelope>,
) -> Result<()> {
    let output_package = output_dir.join("package.json");
    if !output_package.exists() {
        return Ok(());
    }

    let mut output_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&output_package).await?)?;

    let mut sources: Vec<(String, PathBuf)> =
        vec![("the application".to_string(), app_dir.to_path_buf())];
    if let Some(envelope) = metadata {
        for plugin in &envelope.plugins {
            if let Some(frontend) = plugin.frontend_path.as_deref() {
                if let Some(crate_dir) = Path::new(frontend).parent() {
                    sources.push((format!("plugin `{}`", plugin.name), crate_dir.to_path_buf()));
                }
            }
        }
    }

    // Remembers who asked for each requirement so a conflict can name both sides.
    let mut declared_by: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    if let Some(existing) = output_json.get("dependencies").and_then(|d| d.as_object()) {
        for (name, requirement) in existing {
            declared_by.insert(
                name.clone(),
                (
                    "the app template".to_string(),
                    requirement.as_str().unwrap_or_default().to_string(),
                ),
            );
        }
    }

    let mut added = 0usize;
    for (label, dir) in sources {
        let manifest = dir.join("package.json");
        if !manifest.exists() {
            continue;
        }

        let source_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest).await?)?;
        let Some(deps) = source_json.get("dependencies").and_then(|d| d.as_object()) else {
            continue;
        };

        for (name, requirement) in deps {
            let requirement_str = requirement.as_str().unwrap_or_default().to_string();

            if let Some((owner, existing)) = declared_by.get(name) {
                if existing != &requirement_str {
                    anyhow::bail!(
                        "frontend dependency `{name}` is required as `{existing}` by {owner} \
                         and as `{requirement_str}` by {label}. Align the two declarations, \
                         or drop the redundant one if the template already provides it."
                    );
                }
                continue;
            }

            debug!("Adding dependency from {label}: {name} = {requirement_str}");
            if let Some(target) = output_json
                .get_mut("dependencies")
                .and_then(|d| d.as_object_mut())
            {
                target.insert(name.clone(), requirement.clone());
                added += 1;
            }
            declared_by.insert(name.clone(), (label.clone(), requirement_str));
        }
    }

    fs::write(&output_package, serde_json::to_string_pretty(&output_json)?).await?;
    if added > 0 {
        info!(added, "Merged frontend dependencies");
    }

    Ok(())
}

/// Link frontend using PROXY mode - creates re-export files that import from source
/// This enables instant HMR since Vite watches the original source files
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

/// Write the manifests the frontend reads at build time.
///
/// Route access rules are deliberately absent: they are compiled into the binary
/// by `yeollin_plugin!`, so emitting them here as well would create a second
/// copy that nothing reads and that could silently disagree.
async fn write_manifests(output_dir: &Path, metadata: Option<&ExportEnvelope>) -> Result<()> {
    let src_dir = output_dir.join("src");

    let (menu_items, plugins) = match metadata {
        Some(envelope) => (
            envelope
                .menus
                .iter()
                .flat_map(|config| config.items.iter().cloned())
                .collect(),
            envelope.plugins.clone(),
        ),
        None => (vec![], vec![]),
    };

    fs::write(
        src_dir.join("menus.json"),
        serde_json::to_string_pretty(&menu_items)?,
    )
    .await?;
    fs::write(
        src_dir.join("plugins.json"),
        serde_json::to_string_pretty(&plugins)?,
    )
    .await?;

    info!(
        menus = menu_items.len(),
        plugins = plugins.len(),
        "Wrote generated manifests"
    );
    Ok(())
}

/// Copy plugin frontend files
async fn copy_plugin_frontends(
    output_dir: &Path,
    metadata: Option<&ExportEnvelope>,
) -> Result<bool> {
    let Some(envelope) = metadata else {
        return Ok(false);
    };

    let mut copied_any = false;

    for plugin in &envelope.plugins {
        let name = plugin.name.as_str();
        let dest_base = output_dir.join("src").join("app").join("(auth)").join(name);

        if let Some(settings) = &plugin.settings {
            let settings_dir = dest_base.join("settings");
            if settings.custom_page {
                let frontend_path = plugin.frontend_path.as_deref().with_context(|| {
                    format!("plugin `{name}` declares a custom settings page without a frontend")
                })?;
                let source = Path::new(frontend_path).join("settings");
                if !source.is_dir() {
                    anyhow::bail!(
                        "plugin `{name}` declares a custom settings page, but {} is missing",
                        source.display()
                    );
                }
                copy_dir_contents_parallel(&source, &settings_dir).await?;
                info!(plugin = name, "Copied custom plugin settings page");
            } else {
                write_generated_settings_page(&settings_dir, name, settings).await?;
                info!(plugin = name, "Generated plugin settings page");
            }
            copied_any = true;
        }

        let Some(frontend_path) = plugin.frontend_path.as_deref() else {
            continue;
        };

        let frontend_dir = Path::new(frontend_path);
        if !frontend_dir.exists() || !frontend_dir.is_dir() {
            debug!("Plugin {} frontend path not found: {}", name, frontend_path);
            continue;
        }

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

async fn write_generated_settings_page(
    settings_dir: &Path,
    plugin_name: &str,
    settings: &PluginSettingsInfo,
) -> Result<()> {
    fs::create_dir_all(settings_dir).await?;
    let plugin_name = serde_json::to_string(plugin_name)?;
    let api_path = serde_json::to_string(&settings.api_path)?;
    let schema = serde_json::to_string_pretty(&settings.schema)?;
    let default_value = serde_json::to_string_pretty(&settings.default_value)?;
    let content = format!(
        r#"import {{ PluginSettingsForm, type SettingsSchema }} from '@/components/settings/PluginSettingsForm'

const schema = {schema} as SettingsSchema
const defaultValue = {default_value} as Record<string, unknown>

export default function SettingsPage() {{
  return (
    <PluginSettingsForm
      apiPath={api_path}
      defaultValue={{defaultValue}}
      pluginName={plugin_name}
      schema={{schema}}
    />
  )
}}
"#
    );
    fs::write(settings_dir.join("page.tsx"), content).await?;
    Ok(())
}

/// Copy contents of a directory to destination with parallel file copying
pub(super) async fn copy_dir_contents_parallel(src: &Path, dst: &Path) -> Result<()> {
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

#[cfg(test)]
mod export_tests {
    use super::parse_export_envelope;
    use yeollin_core::EXPORT_SCHEMA_VERSION;

    fn envelope(schema_version: u32) -> String {
        format!(
            r#"{{"schemaVersion":{schema_version},"plugins":[{{"name":"memo","version":"0.1.0"}}],"menus":[],"routes":[{{"path":"/memo","label":"Memo","order":50,"access":"authenticated","menu":true}}]}}"#
        )
    }

    #[test]
    fn accepts_a_lone_envelope() {
        let parsed = parse_export_envelope(&envelope(EXPORT_SCHEMA_VERSION)).unwrap();

        assert_eq!(parsed.schema_version, EXPORT_SCHEMA_VERSION);
        assert_eq!(parsed.plugins.len(), 1);
        assert_eq!(parsed.plugins[0].name, "memo");
        assert_eq!(parsed.routes.len(), 1);
        assert_eq!(parsed.routes[0].path, "/memo");
    }

    #[test]
    fn tolerates_surrounding_whitespace_only() {
        let padded = format!("\n  {}  \n", envelope(EXPORT_SCHEMA_VERSION));
        assert!(parse_export_envelope(&padded).is_ok());
    }

    #[test]
    fn rejects_log_polluted_stdout() {
        let polluted = format!("INFO starting up\n{}", envelope(EXPORT_SCHEMA_VERSION));

        let error = parse_export_envelope(&polluted).unwrap_err().to_string();
        assert!(
            error.contains("logs must go to stderr"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_trailing_output_after_the_envelope() {
        let polluted = format!("{}\nINFO shutting down", envelope(EXPORT_SCHEMA_VERSION));
        assert!(parse_export_envelope(&polluted).is_err());
    }

    #[test]
    fn rejects_empty_stdout() {
        assert!(parse_export_envelope("   \n ").is_err());
    }

    #[test]
    fn rejects_a_bare_json_array() {
        // The shape the old heuristic scanned for must no longer be accepted.
        assert!(parse_export_envelope(r#"[{"name":"memo"}]"#).is_err());
    }

    #[test]
    fn rejects_a_mismatched_schema_version() {
        let error = parse_export_envelope(&envelope(EXPORT_SCHEMA_VERSION + 1))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("schema version"),
            "unexpected error: {error}"
        );
    }
}

#[cfg(test)]
mod assembly_tests {
    use super::{run_prebuild, AppFrontend};
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    use yeollin_core::{
        ExportEnvelope, PluginInfo, PluginSettingsInfo, EXPORT_SCHEMA_VERSION,
    };

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn page(root: &Path, relative: &str) {
        write(
            &root.join(relative).join("page.tsx"),
            "export default function Page() { return null }",
        );
    }

    fn plugin(name: &str, dir: &Path) -> PluginInfo {
        PluginInfo {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            author: None,
            description: None,
            license: None,
            frontend_path: Some(dir.to_string_lossy().into_owned()),
            settings: None,
        }
    }

    /// One host app plus two plugin frontends, all inside a temp directory so
    /// the assembly never depends on the process working directory.
    fn fixture(tmp: &TempDir) -> (PathBuf, PathBuf, AppFrontend, ExportEnvelope) {
        let root = tmp.path();
        let app_dir = root.join("host");
        let output_dir = root.join("out");

        write(&app_dir.join("package.json"), r#"{"dependencies":{}}"#);
        page(&app_dir.join("app"), "(main)");

        // Mirrors a real plugin crate: the manifest sits beside `app/`, which is
        // what `frontend_path` points at.
        let alpha = root.join("plugin-alpha").join("app");
        page(&alpha, "(alpha)");
        page(&alpha, "(alpha)/items");

        let beta = root.join("plugin-beta").join("app");
        page(&beta, "(beta)");

        let frontend = AppFrontend {
            name: "host".to_string(),
            app_path: app_dir.join("app"),
        };

        let metadata = ExportEnvelope {
            schema_version: EXPORT_SCHEMA_VERSION,
            plugins: vec![plugin("plugin-alpha", &alpha), plugin("plugin-beta", &beta)],
            menus: vec![],
            routes: vec![],
        };

        (app_dir, output_dir, frontend, metadata)
    }

    fn tree(root: &Path) -> Vec<String> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(rel) = path.strip_prefix(root) {
                    found.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        found.sort();
        found
    }

    #[tokio::test]
    async fn prebuild_assembles_two_plugins() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);

        run_prebuild(
            &output_dir,
            &app_dir,
            Some(&frontend),
            Some(&metadata),
            true,
            false,
        )
        .await
        .expect("prebuild must succeed");

        let files = tree(&output_dir);
        for expected in [
            "src/menus.json",
            "src/plugins.json",
            "src/app/(auth)/layout.tsx",
            "src/app/(auth)/plugin-alpha/page.tsx",
            "src/app/(auth)/plugin-alpha/items/page.tsx",
            "src/app/(auth)/plugin-beta/page.tsx",
        ] {
            assert!(
                files.contains(&expected.to_string()),
                "missing {expected} in assembled output:\n{}",
                files.join("\n")
            );
        }

        let plugins: Vec<PluginInfo> = serde_json::from_str(
            &std::fs::read_to_string(output_dir.join("src").join("plugins.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].name, "plugin-alpha");

        // Access rules are compiled into the binary, not emitted here, so a
        // second copy must never reappear in the generated app.
        assert!(
            !files.iter().any(|file| file.contains("route-manifest")),
            "prebuild must not emit a route manifest"
        );
    }

    #[tokio::test]
    async fn settings_pages_are_generated_or_overridden_per_plugin() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, mut metadata) = fixture(&tmp);
        metadata.plugins[0].settings = Some(PluginSettingsInfo {
            schema: serde_json::json!({
                "type": "object",
                "properties": { "enabled": { "type": "boolean" } }
            }),
            default_value: serde_json::json!({ "enabled": false }),
            api_path: "/api/plugin-alpha/settings".to_string(),
            page_path: "/plugin-alpha/settings".to_string(),
            custom_page: false,
        });
        metadata.plugins[1].settings = Some(PluginSettingsInfo {
            schema: serde_json::json!({ "type": "object" }),
            default_value: serde_json::json!({}),
            api_path: "/api/plugin-beta/settings".to_string(),
            page_path: "/plugin-beta/settings".to_string(),
            custom_page: true,
        });
        let beta_frontend = PathBuf::from(metadata.plugins[1].frontend_path.as_ref().unwrap());
        write(
            &beta_frontend.join("settings").join("page.tsx"),
            "export default function CustomSettings() { return null }",
        );

        run_prebuild(
            &output_dir,
            &app_dir,
            Some(&frontend),
            Some(&metadata),
            true,
            false,
        )
        .await
        .unwrap();

        let generated = std::fs::read_to_string(
            output_dir
                .join("src/app/(auth)/plugin-alpha/settings/page.tsx"),
        )
        .unwrap();
        assert!(generated.contains("PluginSettingsForm"));
        assert!(generated.contains("/api/plugin-alpha/settings"));
        assert!(generated.contains("enabled"));

        let custom = std::fs::read_to_string(
            output_dir.join("src/app/(auth)/plugin-beta/settings/page.tsx"),
        )
        .unwrap();
        assert!(custom.contains("CustomSettings"));
        assert!(!custom.contains("PluginSettingsForm"));
    }

    #[tokio::test]
    async fn prebuild_is_deterministic_across_runs() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);

        run_prebuild(
            &output_dir,
            &app_dir,
            Some(&frontend),
            Some(&metadata),
            true,
            false,
        )
        .await
        .unwrap();
        let first = tree(&output_dir);

        run_prebuild(
            &output_dir,
            &app_dir,
            Some(&frontend),
            Some(&metadata),
            true,
            false,
        )
        .await
        .unwrap();
        let second = tree(&output_dir);

        assert_eq!(first, second, "re-running prebuild changed the output tree");
    }

    #[tokio::test]
    async fn prebuild_without_metadata_emits_empty_manifests() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, _) = fixture(&tmp);

        run_prebuild(&output_dir, &app_dir, Some(&frontend), None, true, false)
            .await
            .unwrap();

        let plugins = std::fs::read_to_string(output_dir.join("src").join("plugins.json")).unwrap();
        assert_eq!(plugins.trim(), "[]");

        let files = tree(&output_dir);
        assert!(
            !files.iter().any(|f| f.contains("plugin-alpha")),
            "no plugin frontend should be copied without metadata"
        );
    }

    fn manifest_of(tmp: &TempDir, plugin: &str, contents: &str) {
        write(&tmp.path().join(plugin).join("package.json"), contents);
    }

    fn dependencies_of(output_dir: &Path) -> serde_json::Value {
        let raw = std::fs::read_to_string(output_dir.join("package.json")).unwrap();
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["dependencies"].clone()
    }

    async fn assemble(
        tmp: &TempDir,
        app_dir: &Path,
        output_dir: &Path,
        frontend: &AppFrontend,
        metadata: &ExportEnvelope,
    ) -> anyhow::Result<()> {
        // The template supplies the base dependency set the plugins merge into.
        write(
            &output_dir.join("package.json"),
            r#"{"dependencies":{"react":"^19.2.8"},"devDependencies":{}}"#,
        );
        let _ = tmp;
        run_prebuild(
            output_dir,
            app_dir,
            Some(frontend),
            Some(metadata),
            false,
            false,
        )
        .await
    }

    #[tokio::test]
    async fn plugin_dependencies_reach_the_generated_app() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);
        manifest_of(
            &tmp,
            "plugin-alpha",
            r#"{"dependencies":{"dayjs":"^1.11.23"}}"#,
        );

        assemble(&tmp, &app_dir, &output_dir, &frontend, &metadata)
            .await
            .expect("prebuild must succeed");

        assert_eq!(dependencies_of(&output_dir)["dayjs"], "^1.11.23");
    }

    #[tokio::test]
    async fn plugin_dev_dependencies_stay_local() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);
        manifest_of(
            &tmp,
            "plugin-alpha",
            r#"{"devDependencies":{"typescript":"^7.0"}}"#,
        );

        assemble(&tmp, &app_dir, &output_dir, &frontend, &metadata)
            .await
            .unwrap();

        // A plugin's devDependencies configure its own editor and typecheck run;
        // the assembled app takes its tooling from the template.
        assert!(dependencies_of(&output_dir)["typescript"].is_null());
    }

    #[tokio::test]
    async fn identical_requirements_merge_quietly() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);
        manifest_of(
            &tmp,
            "plugin-alpha",
            r#"{"dependencies":{"dayjs":"^1.11.23"}}"#,
        );
        manifest_of(
            &tmp,
            "plugin-beta",
            r#"{"dependencies":{"dayjs":"^1.11.23"}}"#,
        );

        assemble(&tmp, &app_dir, &output_dir, &frontend, &metadata)
            .await
            .expect("agreeing plugins must merge");

        assert_eq!(dependencies_of(&output_dir)["dayjs"], "^1.11.23");
    }

    #[tokio::test]
    async fn conflicting_requirements_fail_with_both_sides_named() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);
        manifest_of(
            &tmp,
            "plugin-alpha",
            r#"{"dependencies":{"dayjs":"^1.11.23"}}"#,
        );
        manifest_of(
            &tmp,
            "plugin-beta",
            r#"{"dependencies":{"dayjs":"^2.0.0"}}"#,
        );

        let error = assemble(&tmp, &app_dir, &output_dir, &frontend, &metadata)
            .await
            .expect_err("disagreeing plugins must not silently resolve")
            .to_string();

        assert!(error.contains("dayjs"), "unexpected error: {error}");
        assert!(error.contains("plugin-alpha"), "unexpected error: {error}");
        assert!(error.contains("plugin-beta"), "unexpected error: {error}");
    }

    #[tokio::test]
    async fn a_plugin_may_not_contradict_the_template() {
        let tmp = TempDir::new().unwrap();
        let (app_dir, output_dir, frontend, metadata) = fixture(&tmp);
        manifest_of(
            &tmp,
            "plugin-alpha",
            r#"{"dependencies":{"react":"^18.0.0"}}"#,
        );

        let error = assemble(&tmp, &app_dir, &output_dir, &frontend, &metadata)
            .await
            .expect_err("a plugin must not silently downgrade a template dependency")
            .to_string();

        assert!(error.contains("react"), "unexpected error: {error}");
    }
}
