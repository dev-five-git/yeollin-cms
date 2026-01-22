//! Example CMS Application
//!
//! Demonstrates how to build a CMS using Yeollin with:
//! - Built-in plugin (from lib.rs)
//! - External plugin (example-plugin crate)
//! - Embedded static files in release builds

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Embed static files from Next.js SSG output in release builds
// Import the module itself so the macro expansion can find include_dir::Dir, include_dir::File, etc.
#[cfg(not(debug_assertions))]
use yeollin_plugin::include_dir;

#[cfg(not(debug_assertions))]
static STATIC_DIR: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../.yeollin/app/out");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_app=debug,yeollin=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Example CMS Application");

    let mut builder = yeollin::app()
        .host("0.0.0.0")
        .port(3001)
        // Built-in plugin (from this crate's lib.rs)
        .register_plugin(example_app::metadata())
        // External plugin (separate crate)
        .register_plugin(example_plugin::metadata());

    // Add embedded static files for production (release builds)
    #[cfg(not(debug_assertions))]
    {
        builder = builder.with_static(&STATIC_DIR);
        tracing::info!("Embedded static files enabled (release build)");
    }

    let app = builder.build();

    tracing::info!(menus = %app.export_menus_json(), "Registered menus");

    app.run().await
}
