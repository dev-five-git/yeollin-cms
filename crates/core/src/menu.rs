//! Menu configuration types

use serde::{Deserialize, Serialize};
use vespera::Schema;

/// Menu item for CMS navigation
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MenuItem {
    /// Unique identifier for the menu item
    pub id: String,
    /// Display label
    pub label: String,
    /// Icon name (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Route path
    pub path: String,
    /// Sort order
    #[serde(default)]
    pub order: i32,
    /// Child menu items
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<MenuItem>,
}

/// Menu configuration from plugin
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MenuConfig {
    /// Plugin name that owns this menu
    pub plugin: String,
    /// Menu items
    pub items: Vec<MenuItem>,
}
