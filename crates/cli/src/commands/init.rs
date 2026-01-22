//! Init command
//!
//! Creates a new Yeollin plugin with both API and frontend scaffolding.

use std::path::PathBuf;
use std::fs;
use clap::Args;
use anyhow::Result;
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
    let project_dir = args.project_dir
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));
    
    let plugin_name = sanitize_name(&args.name);
    let app_name = args.app_name.map(|n| sanitize_name(&n)).unwrap_or_else(|| plugin_name.clone());
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

    fs::create_dir_all(&api_routes_dir)?;
    fs::create_dir_all(&app_plugin_dir)?;

    // Generate API files
    generate_api_cargo_toml(&api_dir, &app_name, &args.description)?;
    generate_api_lib_rs(&api_src_dir, &plugin_name, &args.description)?;
    generate_api_routes_mod(&api_src_dir)?;
    generate_api_routes_api_mod(&api_src_dir.join("routes").join("api"), &plugin_name)?;
    generate_api_routes_plugin_mod(&api_routes_dir, &plugin_name)?;

    // Generate frontend files (simplified - no standalone Next.js setup)
    generate_package_json(&plugin_dir, &app_name)?;
    generate_app_page_tsx(&app_plugin_dir, &plugin_name)?;

    info!("Plugin '{}' created at {}", plugin_name, plugin_dir.display());
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
    info!("     .register_plugin({}::metadata())", app_name.replace('-', "_"));
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
// API Generation
// ============================================================================

fn generate_api_cargo_toml(api_dir: &PathBuf, name: &str, description: &str) -> Result<()> {
    let content = format!(r#"[package]
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
"#);
    
    fs::write(api_dir.join("Cargo.toml"), content)?;
    Ok(())
}

fn generate_api_lib_rs(src_dir: &PathBuf, name: &str, description: &str) -> Result<()> {
    let content = format!(r#"//! {description}

mod routes;

yeollin_plugin::yeollin_plugin! {{
    name: "{name}",
    description: "{description}",
}}
"#);
    
    fs::write(src_dir.join("lib.rs"), content)?;
    Ok(())
}

fn generate_api_routes_mod(src_dir: &PathBuf) -> Result<()> {
    let content = r#"//! Route handlers

pub mod api;
"#;
    
    fs::write(src_dir.join("routes").join("mod.rs"), content)?;
    Ok(())
}

fn generate_api_routes_api_mod(api_dir: &PathBuf, name: &str) -> Result<()> {
    let rust_name = to_rust_ident(name);
    let content = format!(r#"//! API routes

pub mod {rust_name};
"#);
    
    fs::write(api_dir.join("mod.rs"), content)?;
    Ok(())
}

fn generate_api_routes_plugin_mod(plugin_routes_dir: &PathBuf, name: &str) -> Result<()> {
    let pascal_name = to_pascal_case(name);
    let content = format!(r#"//! /{name} API routes

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
"#);
    
    fs::write(plugin_routes_dir.join("mod.rs"), content)?;
    Ok(())
}

// ============================================================================
// Frontend Generation
// ============================================================================

fn generate_package_json(plugin_dir: &PathBuf, name: &str) -> Result<()> {
    let content = format!(r#"{{
  "name": "@yeollin-plugin/{name}",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "dev": "cargo run -p yeollin-cli -- dev",
    "build": "cargo run -p yeollin-cli -- build"
  }}
}}
"#);
    
    fs::write(plugin_dir.join("package.json"), content)?;
    Ok(())
}

fn generate_app_page_tsx(plugin_app_dir: &PathBuf, name: &str) -> Result<()> {
    let pascal_name = to_pascal_case(name);
    let content = format!(r#""use client";

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
"#);
    
    fs::write(plugin_app_dir.join("page.tsx"), content)?;
    Ok(())
}
