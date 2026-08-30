//! Item routes, mounted under the plugin API namespace.

use serde::{Deserialize, Serialize};
use vespera::axum::{extract::Path, Json};
use vespera::Schema;

/// Example item
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ExampleItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: String,
}

/// List all items
#[vespera::route(get, path = "/", tags = ["example"])]
pub async fn list() -> Json<Vec<ExampleItem>> {
    Json(vec![
        ExampleItem {
            id: "1".to_string(),
            name: "First Item".to_string(),
            description: Some("This is the first example item".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        },
        ExampleItem {
            id: "2".to_string(),
            name: "Second Item".to_string(),
            description: None,
            created_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ])
}

/// Get a single item by ID
#[vespera::route(get, path = "/{id}", tags = ["example"])]
pub async fn get(
    Path(id): Path<String>,
) -> Result<Json<ExampleItem>, vespera::axum::http::StatusCode> {
    if id == "1" {
        Ok(Json(ExampleItem {
            id: "1".to_string(),
            name: "First Item".to_string(),
            description: Some("This is the first example item".to_string()),
            created_at: "2024-01-01T00:00:00Z".to_string(),
        }))
    } else {
        Err(vespera::axum::http::StatusCode::NOT_FOUND)
    }
}
