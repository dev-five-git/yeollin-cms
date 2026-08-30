//! Example Memo Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with database CRUD operations.
//! It uses sea-orm for database access and vespera for OpenAPI routes.
//!
//! Note: on_init is auto-generated because vespertide.json exists in this plugin.

pub mod models;
pub mod routes;

use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{DatabaseConnection, EventEnvelope, SubscriberRegistration};

/// Settings rendered by the framework's schema-generated form.
#[derive(Debug, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MemoSettings {
    pub compact_mode: bool,
    pub footer_note: String,
}

async fn log_memo_change(event: EventEnvelope, _db: DatabaseConnection) -> anyhow::Result<()> {
    tracing::info!(event_id = event.id, event = %event.name, "Observed committed memo event");
    Ok(())
}

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    author: "DevFive",
    description: "Example memo plugin with database CRUD operations",
    settings: MemoSettings,
    subscribers: [SubscriberRegistration::deferred(
        "change-log",
        ["memo.created", "memo.updated", "memo.deleted"],
        log_memo_change,
    )],
}

// Re-export entity for migrations
pub use models::memo;
