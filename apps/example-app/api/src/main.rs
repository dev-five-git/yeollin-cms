//! Example CMS Application
//!
//! Standalone CMS using Yeollin with Vespera for API routes.

mod routes;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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

    // Create vespera router with OpenAPI docs
    let vespera_router = vespera::vespera!(
        openapi = "openapi.json",
        title = "Example CMS API",
        version = "1.0.0",
        docs_url = "/docs",
        redoc_url = "/redoc"
    );

    let app = yeollin::app()
        .host("0.0.0.0")
        .port(port)
        .merge(vespera_router)
        .register_plugin(example_plugin::metadata())
        .build();

    tracing::info!(menus = %app.export_menus_json(), "Registered menus");

    app.run().await
}
