//! Plugin registration commands.
//!
//! Cargo resolves the dependency graph before proc macros run, so a plugin
//! cannot be discovered at compile time — the application must declare it as a
//! dependency *and* list it in `yeollin_app!`. Missing either one fails to
//! compile or silently omits the plugin, so both edits belong to one command.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use tokio::fs;
use tracing::{info, warn};

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommand,
}

#[derive(Subcommand)]
pub enum PluginCommand {
    /// Register a plugin with an application
    Add(AddArgs),
    /// Report plugins that are half-registered
    Doctor(DoctorArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Plugin crate name, for example `media-library`
    pub name: String,

    /// Application directory (default: current directory)
    #[arg(long)]
    pub app: Option<PathBuf>,
}

#[derive(Args)]
pub struct DoctorArgs {
    /// Application directory (default: current directory)
    #[arg(long)]
    pub app: Option<PathBuf>,
}

pub async fn run(args: PluginArgs) -> Result<()> {
    match args.command {
        PluginCommand::Add(args) => add(args).await,
        PluginCommand::Doctor(args) => doctor(args).await,
    }
}

/// An application's two registration points, read from disk.
struct Registration {
    app_dir: PathBuf,
    manifest: PathBuf,
    main_rs: PathBuf,
    /// Crate names declared as path dependencies pointing into `plugins/`
    dependencies: Vec<String>,
    /// Module idents listed in `yeollin_app! { plugins: [...] }`
    listed: Vec<String>,
}

fn to_ident(name: &str) -> String {
    name.replace('-', "_")
}

fn resolve_app_dir(app: Option<PathBuf>) -> Result<PathBuf> {
    let dir = match app {
        Some(dir) => dir,
        None => std::env::current_dir()?,
    };

    if !dir.join("Cargo.toml").is_file() {
        anyhow::bail!(
            "{} is not a crate directory (no Cargo.toml). Pass --app <dir>.",
            dir.display()
        );
    }

    Ok(dir)
}

/// Walk up from the application to the manifest that declares `[workspace]`.
fn workspace_root(app_dir: &Path) -> Result<PathBuf> {
    for ancestor in app_dir.ancestors() {
        let manifest = ancestor.join("Cargo.toml");
        if manifest.is_file() {
            let raw = std::fs::read_to_string(&manifest).unwrap_or_default();
            if raw.contains("[workspace]") {
                return Ok(ancestor.to_path_buf());
            }
        }
    }

    anyhow::bail!("could not find a workspace root above {}", app_dir.display())
}

async fn read_registration(app_dir: PathBuf) -> Result<Registration> {
    let manifest = app_dir.join("Cargo.toml");
    let main_rs = app_dir.join("src").join("main.rs");

    let manifest_raw = fs::read_to_string(&manifest)
        .await
        .with_context(|| format!("reading {}", manifest.display()))?;
    let document: toml_edit::DocumentMut = manifest_raw.parse()?;

    let dependencies = document
        .get("dependencies")
        .and_then(|deps| deps.as_table_like())
        .map(|deps| {
            deps.iter()
                .filter(|(_, value)| {
                    value
                        .as_table_like()
                        .and_then(|entry| entry.get("path"))
                        .and_then(|path| path.as_str())
                        .is_some_and(|path| path.replace('\\', "/").contains("/plugins/"))
                })
                .map(|(name, _)| name.to_string())
                .collect()
        })
        .unwrap_or_default();

    let listed = match fs::read_to_string(&main_rs).await {
        Ok(source) => parse_plugin_list(&source),
        Err(_) => vec![],
    };

    Ok(Registration {
        app_dir,
        manifest,
        main_rs,
        dependencies,
        listed,
    })
}

/// Extract the idents from `yeollin_app! { plugins: [a, b] }`.
fn parse_plugin_list(source: &str) -> Vec<String> {
    let Some(start) = source.find("plugins:") else {
        return vec![];
    };
    let after = &source[start + "plugins:".len()..];
    let Some(open) = after.find('[') else {
        return vec![];
    };
    let Some(close) = after[open..].find(']') else {
        return vec![];
    };

    after[open + 1..open + close]
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect()
}

fn replace_plugin_list(source: &str, entries: &[String]) -> Option<String> {
    let start = source.find("plugins:")?;
    let after = &source[start + "plugins:".len()..];
    let open = after.find('[')?;
    let close = after[open..].find(']')?;

    let absolute_open = start + "plugins:".len() + open;
    let absolute_close = absolute_open + close;

    let mut replaced = String::with_capacity(source.len() + 32);
    replaced.push_str(&source[..absolute_open + 1]);
    replaced.push_str(&entries.join(", "));
    replaced.push_str(&source[absolute_close..]);
    Some(replaced)
}

async fn add(args: AddArgs) -> Result<()> {
    let app_dir = resolve_app_dir(args.app)?;
    let root = workspace_root(&app_dir)?;
    let plugin_dir = root.join("plugins").join(&args.name);

    if !plugin_dir.join("Cargo.toml").is_file() {
        anyhow::bail!(
            "no plugin crate at {}. Create it first with `yeollin init {}`.",
            plugin_dir.display(),
            args.name
        );
    }

    let registration = read_registration(app_dir).await?;
    let ident = to_ident(&args.name);

    let mut changed = false;

    if registration.dependencies.contains(&args.name) {
        info!(plugin = %args.name, "Already a dependency");
    } else {
        add_dependency(&registration, &root, &args.name).await?;
        changed = true;
    }

    if registration.listed.contains(&ident) {
        info!(plugin = %ident, "Already listed in yeollin_app!");
    } else {
        add_to_plugin_list(&registration, &ident).await?;
        changed = true;
    }

    if changed {
        info!(plugin = %args.name, "Registered");
    } else {
        info!(plugin = %args.name, "Nothing to do");
    }

    Ok(())
}

async fn add_dependency(registration: &Registration, root: &Path, name: &str) -> Result<()> {
    let raw = fs::read_to_string(&registration.manifest).await?;
    let mut document: toml_edit::DocumentMut = raw.parse()?;

    let relative = relative_plugin_path(&registration.app_dir, root, name);
    let mut entry = toml_edit::InlineTable::new();
    entry.insert("path", relative.into());

    document["dependencies"][name] = toml_edit::value(entry);

    fs::write(&registration.manifest, document.to_string()).await?;
    info!(plugin = %name, "Added dependency");
    Ok(())
}

/// Path from the application crate to a plugin crate, in Cargo's forward-slash form.
fn relative_plugin_path(app_dir: &Path, root: &Path, plugin: &str) -> String {
    let depth = app_dir
        .strip_prefix(root)
        .map(|rest| rest.components().count())
        .unwrap_or(2);

    let up = "../".repeat(depth.max(1));
    format!("{up}plugins/{plugin}")
}

async fn add_to_plugin_list(registration: &Registration, ident: &str) -> Result<()> {
    let source = fs::read_to_string(&registration.main_rs)
        .await
        .with_context(|| format!("reading {}", registration.main_rs.display()))?;

    let mut entries = parse_plugin_list(&source);
    if entries.is_empty() && !source.contains("plugins:") {
        anyhow::bail!(
            "could not find a `yeollin_app! {{ plugins: [...] }}` list in {}",
            registration.main_rs.display()
        );
    }

    entries.push(ident.to_string());
    entries.sort();
    entries.dedup();

    let updated = replace_plugin_list(&source, &entries).with_context(|| {
        format!(
            "could not rewrite the plugin list in {}",
            registration.main_rs.display()
        )
    })?;

    fs::write(&registration.main_rs, updated).await?;
    info!(plugin = %ident, "Added to yeollin_app!");
    Ok(())
}

async fn doctor(args: DoctorArgs) -> Result<()> {
    let app_dir = resolve_app_dir(args.app)?;
    let registration = read_registration(app_dir).await?;

    let mut problems = 0usize;

    for name in &registration.dependencies {
        let ident = to_ident(name);
        if !registration.listed.contains(&ident) {
            warn!(
                plugin = %name,
                "Declared as a dependency but missing from yeollin_app!; its routes and migrations will not load"
            );
            problems += 1;
        }
    }

    for ident in &registration.listed {
        let expected = ident.replace('_', "-");
        if !registration
            .dependencies
            .iter()
            .any(|name| to_ident(name) == *ident)
        {
            warn!(
                plugin = %expected,
                "Listed in yeollin_app! but not declared as a dependency; the crate will not resolve"
            );
            problems += 1;
        }
    }

    let mut seen = std::collections::HashSet::new();
    for ident in &registration.listed {
        if !seen.insert(ident) {
            warn!(plugin = %ident, "Listed twice in yeollin_app!");
            problems += 1;
        }
    }

    if problems == 0 {
        info!(
            plugins = registration.listed.len(),
            "Every plugin is registered on both sides"
        );
        return Ok(());
    }

    anyhow::bail!("{problems} registration problem(s); run `yeollin plugin add <name>` to fix")
}

#[cfg(test)]
mod tests {
    use super::{parse_plugin_list, replace_plugin_list, to_ident};

    const SOURCE: &str = r#"
    let app = yeollin::yeollin_app! {
        plugins: [auth, example_plugin],
        openapi: "openapi.json",
    };
"#;

    #[test]
    fn reads_the_declared_plugins() {
        assert_eq!(parse_plugin_list(SOURCE), vec!["auth", "example_plugin"]);
    }

    #[test]
    fn reads_an_empty_list() {
        assert!(parse_plugin_list("plugins: [],").is_empty());
    }

    #[test]
    fn ignores_a_source_without_a_list() {
        assert!(parse_plugin_list("fn main() {}").is_empty());
    }

    #[test]
    fn rewrites_only_the_list() {
        let updated = replace_plugin_list(
            SOURCE,
            &["auth".to_string(), "example_plugin".to_string(), "memo".to_string()],
        )
        .unwrap();

        assert!(updated.contains("plugins: [auth, example_plugin, memo]"));
        assert!(
            updated.contains(r#"openapi: "openapi.json""#),
            "surrounding macro fields must survive"
        );
    }

    #[test]
    fn crate_names_become_module_idents() {
        assert_eq!(to_ident("media-library"), "media_library");
        assert_eq!(to_ident("auth"), "auth");
    }
}
