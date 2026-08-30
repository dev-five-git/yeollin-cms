//! Example Memo Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with database CRUD operations.
//! It uses sea-orm for database access and vespera for OpenAPI routes.
//!
//! Note: on_init is auto-generated because vespertide.json exists in this plugin.

pub mod models;
pub mod routes;

use serde::{Deserialize, Serialize};
use vespera::Schema;

/// Settings rendered by the framework's schema-generated form.
#[derive(Debug, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MemoSettings {
    pub compact_mode: bool,
    pub footer_note: String,
}

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    author: "DevFive",
    description: "Example memo plugin with database CRUD operations",
    settings: MemoSettings,
}

// Re-export entity for migrations
pub use models::memo;
