//! Yeollin CMS Entry Point

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "yeollin=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Yeollin CMS");

    // Build the application
    let mut builder = yeollin::app()
        .host("0.0.0.0")
        .port(3001);

    // Register example plugin if feature is enabled
    #[cfg(feature = "example-plugin")]
    {
        builder = builder.register_plugin(example_plugin::metadata());
    }

    let app = builder.build();

    // Export menus for frontend build
    let menus_json = app.export_menus_json();
    tracing::debug!(menus = %menus_json, "Registered menus");

    // Run the server
    app.run().await
}
