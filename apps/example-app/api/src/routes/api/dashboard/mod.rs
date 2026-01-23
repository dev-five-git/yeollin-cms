//! /api/dashboard routes

use serde::Serialize;
use vespera::axum::Json;
use vespera::Schema;

/// Dashboard statistics
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_content: u32,
    pub published: u32,
    pub drafts: u32,
    pub views_today: u32,
}

/// Get dashboard statistics
#[vespera::route(get, path = "/stats", tags = ["dashboard"])]
pub async fn stats() -> Json<DashboardStats> {
    Json(DashboardStats {
        total_content: 42,
        published: 35,
        drafts: 7,
        views_today: 1234,
    })
}
