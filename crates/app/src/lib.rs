//! Yeollin CMS Application Library
//!
//! This crate provides the main CMS application builder.
//!
//! # Example
//!
//! ```rust,ignore
//! use yeollin::YeollinApp;
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = yeollin::app()
//!         .register_plugin(my_plugin::metadata())
//!         .build();
//!     
//!     app.run().await;
//! }
//! ```

mod app;
mod dev_proxy;
mod server;
mod state;
mod static_files;

pub use app::*;
pub use yeollin_core::*;
pub use yeollin_plugin::PluginMetadata;
pub use static_files::static_router;
pub use dev_proxy::dev_proxy_router;

/// Create a new Yeollin application builder
pub fn app() -> YeollinAppBuilder {
    YeollinAppBuilder::new()
}
