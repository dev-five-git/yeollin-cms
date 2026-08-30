//! Audit event read API, mounted at `/api/audit-log`.

use axum::{extract::Query, Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vespera::Schema;
use yeollin_plugin::{Authorize, CurrentUser, EventBus, PluginError, SettingsStore};

use crate::{retention_cutoff, AuditLogSettings};

const DEFAULT_PAGE_SIZE: u64 = 20;
const MAX_PAGE_SIZE: u64 = 100;

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListAuditEventsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub event_name: Option<String>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventResponse {
    pub id: i64,
    pub name: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListAuditEventsResponse {
    pub events: Vec<AuditEventResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub retention_days: u32,
}

/// List audit-marked events newest first.
#[vespera::route(get, tags = ["audit-log"])]
pub async fn list_audit_events(
    Extension(events): Extension<EventBus>,
    Extension(settings): Extension<SettingsStore>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<ListAuditEventsQuery>,
) -> Result<Json<ListAuditEventsResponse>, PluginError> {
    current.require_role("admin")?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let event_name = query
        .event_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let settings = settings.get::<AuditLogSettings>("audit-log").await?;

    events
        .purge_audited_before(retention_cutoff(&settings))
        .await?;
    let (event_rows, total) = events
        .audited_events((page - 1) * page_size, page_size, event_name)
        .await?;
    let events = event_rows
        .into_iter()
        .map(|event| AuditEventResponse {
            id: event.id,
            name: event.name,
            payload: event.payload,
            created_at: event.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListAuditEventsResponse {
        events,
        total,
        page,
        page_size,
        retention_days: settings.retention_days,
    }))
}
