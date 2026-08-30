//! Example Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with both
//! backend API routes and frontend UI components.

mod routes;

use serde::{Deserialize, Serialize};
use vespera::Schema;

/// Settings used by the example plugin's custom settings screen.
#[derive(Debug, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ExampleSettings {
    pub homepage_message: String,
    pub maintenance_mode: bool,
}

yeollin_plugin::yeollin_plugin! {
    name: "example-plugin",
    author: "DevFive",
    description: "Example plugin demonstrating Yeollin CMS plugin architecture",
    settings: ExampleSettings,
}

// Re-export types for external use
pub use routes::items::ExampleItem;
