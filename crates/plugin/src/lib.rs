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
//!     description: "My awesome plugin",
//! }
//! ```

mod frontend;
mod macros;
mod metadata;

pub use frontend::*;
pub use metadata::*;

// Re-export for convenience
pub use include_dir;
pub use vespera;
pub use yeollin_auth;
pub use yeollin_core;

// Re-export commonly used auth types
pub use yeollin_auth::{
    auth_middleware, generate_token, verify_token, AuthConfig, AuthError, AuthState, Claims,
    CurrentUser, TokenType,
};
