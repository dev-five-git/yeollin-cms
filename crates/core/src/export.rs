//! The metadata contract between a built application binary and `yeollin-cli`.
//!
//! Prebuild learns what a binary contains by running it in export mode. Keeping
//! the envelope here means the producer and the consumer are type-checked
//! against one definition instead of against a hand-written mirror.

use serde::{Deserialize, Serialize};
use vespera::Schema;

use crate::menu::MenuConfig;
use crate::route::RouteEntry;

/// Setting this environment variable makes the binary print an
/// [`ExportEnvelope`] and exit without serving, connecting to a database, or
/// running plugin initialisation.
pub const EXPORT_ENV_VAR: &str = "YEOLLIN_EXPORT";

/// Version of the [`ExportEnvelope`] contract.
pub const EXPORT_SCHEMA_VERSION: u32 = 2;

/// Build-time information used to expose a plugin's settings screen.
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsInfo {
    /// Exact Vespera-generated JSON schema for the plugin's settings type.
    pub schema: serde_json::Value,
    /// Serialized `Default` value for a newly installed plugin.
    pub default_value: serde_json::Value,
    /// Typed API endpoint generated for this plugin.
    pub api_path: String,
    /// Frontend route where the settings form is assembled.
    pub page_path: String,
    /// Whether the plugin supplies `app/settings/page.tsx` instead of the generated form.
    pub custom_page: bool,
}

/// Plugin information exposed to the CLI and the plugins API.
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<PluginSettingsInfo>,
}

/// Everything prebuild needs from a built binary, in one document.
///
/// Emitted alone on stdout so the reader parses the entire stream rather than
/// scanning it for something that looks like JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEnvelope {
    pub schema_version: u32,
    pub plugins: Vec<PluginInfo>,
    pub menus: Vec<MenuConfig>,
    pub routes: Vec<RouteEntry>,
}
