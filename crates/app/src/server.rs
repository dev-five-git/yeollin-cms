//! HTTP server

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use crate::state::AppState;

pub struct Server {
    router: Router,
    state: AppState,
}

impl Server {
    pub fn new(router: Router, state: AppState) -> Self {
        Self { router, state }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        // Add middleware
        let router = self.router
            .layer(TraceLayer::new_for_http())
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            );

        let addr = self.state.addr();
        tracing::info!("Starting Yeollin CMS server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}
