//! Example Memo Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with database CRUD operations.
//! It uses sea-orm for database access and vespera for OpenAPI routes.

pub mod models;
pub mod routes;

use axum::{routing, Router};
use yeollin_plugin::DatabaseConnection;

/// Create the memo API router
pub fn memo_router() -> Router {
    Router::new()
        .route(
            "/api/memos",
            routing::get(routes::list_memos).post(routes::create_memo),
        )
        .route(
            "/api/memos/{id}",
            routing::get(routes::get_memo)
                .patch(routes::update_memo)
                .delete(routes::delete_memo),
        )
}

/// Plugin initialization - run database migrations
pub async fn on_init(db: DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(&db).await?;
    Ok(())
}

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    author: "DevFive",
    description: "Example memo plugin with database CRUD operations",
    router: memo_router(),
    on_init: on_init,
}

// Re-export entity for migrations
pub use models::memo;
