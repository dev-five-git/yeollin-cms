//! Example Memo Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with database CRUD operations.
//! It uses sea-orm for database access and vespera for OpenAPI routes.

pub mod models;
pub mod routes;

use yeollin_plugin::DatabaseConnection;

/// Plugin initialization - run database migrations
pub async fn on_init(db: DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(&db).await?;
    Ok(())
}

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    author: "DevFive",
    description: "Example memo plugin with database CRUD operations",
    on_init: on_init,
}

// Re-export entity for migrations
pub use models::memo;
