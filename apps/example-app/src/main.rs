//! Example CMS Application
//!
//! Standalone CMS using Yeollin with Vespera for API routes.

mod routes;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use yeollin::AuthConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    "example_app=debug,yeollin=debug,auth=info,tower_http=debug".into()
                }),
        )
        // stdout is reserved for the metadata export envelope, so logs go to stderr.
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting Example CMS Application");

    // Get port from environment or use default
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    // Left empty when unset so that metadata-export runs, which never sign a
    // token, still work without deployment secrets. `YeollinApp::run` rejects a
    // weak secret before it serves traffic.
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();

    // Credentials live in the auth plugin, not here. It seeds the first
    // administrator from YEOLLIN_ADMIN_USERNAME / YEOLLIN_ADMIN_PASSWORD.
    let auth_config = AuthConfig::new(jwt_secret);
    let storage_dir =
        std::env::var("YEOLLIN_STORAGE_DIR").unwrap_or_else(|_| "./storage".to_string());

    // Create app builder using yeollin_app! macro
    // This macro handles both register_plugin() and vespera merge in one call
    let app = yeollin::yeollin_app! {
        plugins: [audit_log, auth, content, example_memo_plugin, example_plugin, media],
        openapi: "openapi.json",
        title: "Example CMS API",
        version: "1.0.0",
        docs_url: "/docs",
        redoc_url: "/redoc",
    }
    .host("0.0.0.0")
    .port(port)
    .with_auth(auth_config)
    .with_storage_dir(storage_dir)
    // `mode=rwc` creates the file on first run; vespertide provisions the schema
    // on plugin init. Connecting lazily keeps metadata exports side-effect free.
    .with_database_url("sqlite://./db.sqlite?mode=rwc")
    .build();

    tracing::info!(menus = %app.export_menus_json(), "Registered menus");

    app.run().await
}
