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
                .unwrap_or_else(|_| "example_app=debug,yeollin=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Example CMS Application");

    // Get port from environment or use default
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    // Get JWT secret from environment or use default (CHANGE IN PRODUCTION!)
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "yeollin-cms-secret-key-change-in-production".to_string());

    // Get superadmin credentials from environment
    let superadmin_username =
        std::env::var("SUPERADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let superadmin_password =
        std::env::var("SUPERADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());

    // Create auth config
    let auth_config =
        AuthConfig::new(jwt_secret).superadmin(superadmin_username.clone(), superadmin_password);

    tracing::info!(username = %superadmin_username, "Superadmin configured");

    // Create vespera router with OpenAPI docs
    // Public routes under (public) directory are auto-detected from prebuild
    let app = yeollin::app()
        .host("0.0.0.0")
        .port(port)
        .with_auth(auth_config)
        .register_plugin(example_plugin::metadata())
        .merge(vespera::vespera!(
            openapi = "openapi.json",
            title = "Example CMS API",
            version = "1.0.0",
            docs_url = "/docs",
            redoc_url = "/redoc"
        ))
        .build();

    tracing::info!(menus = %app.export_menus_json(), "Registered menus");

    app.run().await
}
