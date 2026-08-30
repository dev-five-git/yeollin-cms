//! Yeollin CMS Plugin Interface
//!
//! This crate provides the plugin interface for extending Yeollin CMS.
//!
//! # Example
//!
//! ```rust,ignore
//! mod routes;
//!
//! yeollin_plugin::yeollin_plugin! {
//!     name: "my-plugin",
//!     author: "Your Name",
//!     description: "My awesome plugin",
//! }
//!
//! // Auto-generates: vespera::export_app!(MyPlugin);
//! // Plugin name "my-plugin" → export identifier `MyPlugin`
//! ```

mod authorize;
mod error;
mod frontend;
mod metadata;

pub use authorize::*;
pub use error::*;
pub use frontend::*;
pub use metadata::*;

// Re-export yeollin_plugin! proc-macro (auto-infers export name from plugin name)
pub use yeollin_plugin_macros::yeollin_plugin;

// Re-export for convenience
pub use include_dir;
pub use sea_orm;
pub use serde_json;
pub use vespera;
pub use yeollin_auth;
pub use yeollin_core;
pub use yeollin_core::{SettingsError, SettingsRegistration, SettingsStore};

// Re-export commonly used auth types
pub use yeollin_auth::{
    auth_middleware, generate_token, verify_token, AuthConfig, AuthError, AuthState, Claims,
    CurrentUser, TokenType,
};

// Re-export database types for plugin on_init callbacks
pub use sea_orm::DatabaseConnection;
