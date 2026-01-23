//! Init command
//!
//! Creates a new Yeollin plugin with both API and frontend scaffolding.

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

#[derive(Args)]
pub struct InitArgs {
    /// Plugin name (e.g., "blog", "media-library")
    pub name: String,

    /// Project root directory (default: current directory)
    #[arg(short, long)]
    pub project_dir: Option<PathBuf>,

    /// Plugin description
    #[arg(short, long, default_value = "A Yeollin CMS plugin")]
    pub description: String,

    /// App/package name for Cargo.toml and package.json (default: same as plugin name)
    #[arg(short, long)]
    pub app_name: Option<String>,
}

pub async fn run(args: InitArgs) -> Result<()> {
    let project_dir = args
        .project_dir
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    let plugin_name = sanitize_name(&args.name);
    let app_name = args
        .app_name
        .map(|n| sanitize_name(&n))
        .unwrap_or_else(|| plugin_name.clone());
    let plugin_dir = project_dir.join("plugins").join(&plugin_name);

    if plugin_dir.exists() {
        anyhow::bail!("Plugin directory already exists: {}", plugin_dir.display());
    }

    info!("Creating plugin '{}'...", plugin_name);

    // Create directory structure
    let api_dir = plugin_dir.join("api");
    let api_src_dir = api_dir.join("src");
    let api_routes_dir = api_src_dir.join("routes").join("api").join(&plugin_name);
    let app_dir = plugin_dir.join("app");
    let app_plugin_dir = app_dir.join(format!("({})", plugin_name));

    // Create directories in parallel
    tokio::try_join!(
        fs::create_dir_all(&api_routes_dir),
        fs::create_dir_all(&app_plugin_dir),
    )?;

    // Pre-compute all content (CPU-bound, synchronous)
    let cargo_toml_content = generate_cargo_toml_content(&app_name, &args.description);
    let lib_rs_content = generate_lib_rs_content(&plugin_name, &args.description);
    let routes_mod_content = generate_routes_mod_content();
    let routes_api_mod_content = generate_routes_api_mod_content(&plugin_name);
    let routes_plugin_mod_content = generate_routes_plugin_mod_content(&plugin_name);
    let package_json_content = generate_package_json_content(&app_name);
    let page_tsx_content = generate_page_tsx_content(&plugin_name);

    // Write ALL files IN PARALLEL
    tokio::try_join!(
        fs::write(api_dir.join("Cargo.toml"), &cargo_toml_content),
        fs::write(api_src_dir.join("lib.rs"), &lib_rs_content),
        fs::write(
            api_src_dir.join("routes").join("mod.rs"),
            &routes_mod_content
        ),
        fs::write(
            api_src_dir.join("routes").join("api").join("mod.rs"),
            &routes_api_mod_content
        ),
        fs::write(api_routes_dir.join("mod.rs"), &routes_plugin_mod_content),
        fs::write(plugin_dir.join("package.json"), &package_json_content),
        fs::write(app_plugin_dir.join("page.tsx"), &page_tsx_content),
    )?;

    info!(
        "Plugin '{}' created at {}",
        plugin_name,
        plugin_dir.display()
    );
    if app_name != plugin_name {
        info!("  Crate name: {}", app_name);
        info!("  Package name: @yeollin-plugin/{}", app_name);
    }
    info!("");
    info!("Next steps:");
    info!("  1. Add to workspace Cargo.toml:");
    info!("     members = [..., \"plugins/{}/api\"]", plugin_name);
    info!("");
    info!("  2. Register in your app's main.rs:");
    info!(
        "     .register_plugin({}::metadata())",
        app_name.replace('-', "_")
    );
    info!("");
    info!("  3. Start development:");
    info!("     cd plugins/{} && bun run dev", plugin_name);

    Ok(())
}

/// Sanitize plugin name for use as crate/directory name
fn sanitize_name(name: &str) -> String {
    name.to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

/// Convert plugin name to Rust identifier
fn to_rust_ident(name: &str) -> String {
    name.replace('-', "_")
}

/// Convert plugin name to PascalCase
fn to_pascal_case(name: &str) -> String {
    name.split(&['-', '_'][..])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

// ============================================================================
// Content Generation (pure functions, no I/O)
// ============================================================================

fn generate_cargo_toml_content(name: &str, description: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
description = "{description}"

[lib]
path = "src/lib.rs"

[dependencies]
yeollin-plugin = {{ workspace = true }}
vespera = {{ workspace = true }}
serde = {{ workspace = true }}
serde_json = {{ workspace = true }}
"#
    )
}

fn generate_lib_rs_content(name: &str, description: &str) -> String {
    format!(
        r#"//! {description}

mod routes;

yeollin_plugin::yeollin_plugin! {{
    name: "{name}",
    description: "{description}",
}}
"#
    )
}

fn generate_routes_mod_content() -> String {
    r#"//! Route handlers

pub mod api;
"#
    .to_string()
}

fn generate_routes_api_mod_content(name: &str) -> String {
    let rust_name = to_rust_ident(name);
    format!(
        r#"//! API routes

pub mod {rust_name};
"#
    )
}

fn generate_routes_plugin_mod_content(name: &str) -> String {
    let pascal_name = to_pascal_case(name);
    format!(
        r#"//! /{name} API routes

use serde::{{Deserialize, Serialize}};
use vespera::axum::Json;
use vespera::Schema;

/// {pascal_name} item
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct {pascal_name}Item {{
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
}}

/// List all items
#[vespera::route(get, path = "/", tags = ["{name}"])]
pub async fn list() -> Json<Vec<{pascal_name}Item>> {{
    Json(vec![
        {pascal_name}Item {{
            id: "1".to_string(),
            name: "Example Item".to_string(),
            description: Some("This is an example item".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }},
    ])
}}

/// Get a single item by ID
#[vespera::route(get, path = "/{{id}}", tags = ["{name}"])]
pub async fn get(
    vespera::axum::extract::Path(id): vespera::axum::extract::Path<String>,
) -> Result<Json<{pascal_name}Item>, vespera::axum::http::StatusCode> {{
    if id == "1" {{
        Ok(Json({pascal_name}Item {{
            id: "1".to_string(),
            name: "Example Item".to_string(),
            description: Some("This is an example item".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }}))
    }} else {{
        Err(vespera::axum::http::StatusCode::NOT_FOUND)
    }}
}}
"#
    )
}

fn generate_package_json_content(name: &str) -> String {
    format!(
        r#"{{
  "name": "@yeollin-plugin/{name}",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "dev": "cargo run -p yeollin-cli -- dev",
    "build": "cargo run -p yeollin-cli -- build"
  }}
}}
"#
    )
}

fn generate_page_tsx_content(name: &str) -> String {
    let pascal_name = to_pascal_case(name);
    format!(
        r#""use client";

import {{ Box, Flex, Text }} from "@devup-ui/react";

export default function {pascal_name}Page() {{
  return (
    <Box p={{6}}>
      <Text typography="heading" mb={{4}}>
        {pascal_name}
      </Text>
      <Flex flexDirection="column" gap={{4}}>
        <Box bg="$background" p={{4}} borderRadius="8px" border="1px solid $border">
          <Text typography="subheading" mb={{2}}>
            Welcome
          </Text>
          <Text color="$textSecondary">
            This is the {name} plugin. Start building your feature here.
          </Text>
        </Box>
      </Flex>
    </Box>
  );
}}
"#
    )
}
