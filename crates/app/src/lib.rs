//! Yeollin CMS Application Library
//!
//! This crate provides the main CMS application builder.
//!
//! # Example
//!
//! ```rust,ignore
//! use yeollin::{app, AuthConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = yeollin::app()
//!         .with_auth(
//!             AuthConfig::new("secret")
//!                 .superadmin("admin", "password")
//!         )
//!         .register_plugin(my_plugin::metadata())
//!         .build();
//!     
//!     app.run().await;
//! }
//! ```

mod app;
mod auth_routes;
mod dev_proxy;
mod server;
mod state;
mod static_files;

pub use app::*;
pub use dev_proxy::dev_proxy_router;
pub use static_files::static_router;
pub use yeollin_core::*;
pub use yeollin_plugin::PluginMetadata;

// Re-export auth utilities for plugins and apps
pub use yeollin_auth::{
    auth_middleware, generate_access_token, generate_token, hash_password, verify_password,
    verify_token, AuthConfig, AuthError, AuthState, Claims, CurrentUser, SuperadminConfig,
    TokenPair, TokenType,
};

// Re-export database utilities for plugins and apps
pub use sea_orm::{Database, DatabaseConnection};

/// Create a new Yeollin application builder
pub fn app() -> YeollinAppBuilder {
    YeollinAppBuilder::new()
}
