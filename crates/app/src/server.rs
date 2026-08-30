//! HTTP server

use crate::state::AppState;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

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
        let router = self.router.layer(TraceLayer::new_for_http()).layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

        let addr = self.state.addr();
        tracing::info!("Starting Yeollin CMS server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        // Connect info is published so handlers can rate-limit per source address.
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}
