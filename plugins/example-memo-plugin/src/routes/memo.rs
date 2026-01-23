//! Memo CRUD routes

use axum::{extract::Path, Extension, Json};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use vespera::Schema;

use crate::entities::{memo, Memo};

/// Memo response
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MemoResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<memo::Model> for MemoResponse {
    fn from(model: memo::Model) -> Self {
        Self {
            id: model.id,
            title: model.title,
            content: model.content,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

/// Create memo request
#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct CreateMemoRequest {
    pub title: String,
    pub content: String,
}

/// Update memo request
#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemoRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

/// List memos response
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListMemosResponse {
    pub memos: Vec<MemoResponse>,
    pub total: u64,
}

/// Error response
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

/// List all memos
#[vespera::route(get, path = "/api/memos", tags = ["memo"])]
pub async fn list_memos(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<ListMemosResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let memos = Memo::find()
        .order_by_desc(memo::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                    code: "DB_ERROR".to_string(),
                }),
            )
        })?;

    let total = memos.len() as u64;
    let memos = memos.into_iter().map(MemoResponse::from).collect();

    Ok(Json(ListMemosResponse { memos, total }))
}

/// Get a memo by ID
#[vespera::route(get, path = "/api/memos/{id}", tags = ["memo"])]
pub async fn get_memo(
    Extension(db): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> Result<Json<MemoResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let memo = Memo::find_by_id(id).one(&db).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "DB_ERROR".to_string(),
            }),
        )
    })?;

    match memo {
        Some(memo) => Ok(Json(MemoResponse::from(memo))),
        None => Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Memo not found".to_string(),
                code: "NOT_FOUND".to_string(),
            }),
        )),
    }
}

/// Create a new memo
#[vespera::route(post, path = "/api/memos", tags = ["memo"])]
pub async fn create_memo(
    Extension(db): Extension<DatabaseConnection>,
    Json(req): Json<CreateMemoRequest>,
) -> Result<Json<MemoResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let now = chrono::Utc::now();
    let memo = memo::ActiveModel {
        title: Set(req.title),
        content: Set(req.content),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let memo = memo.insert(&db).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "DB_ERROR".to_string(),
            }),
        )
    })?;

    Ok(Json(MemoResponse::from(memo)))
}

/// Update a memo
#[vespera::route(patch, path = "/api/memos/{id}", tags = ["memo"])]
pub async fn update_memo(
    Extension(db): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateMemoRequest>,
) -> Result<Json<MemoResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let memo = Memo::find_by_id(id).one(&db).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "DB_ERROR".to_string(),
            }),
        )
    })?;

    let memo = match memo {
        Some(memo) => memo,
        None => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Memo not found".to_string(),
                    code: "NOT_FOUND".to_string(),
                }),
            ))
        }
    };

    let mut memo: memo::ActiveModel = memo.into();
    if let Some(title) = req.title {
        memo.title = Set(title);
    }
    if let Some(content) = req.content {
        memo.content = Set(content);
    }
    memo.updated_at = Set(chrono::Utc::now());

    let memo = memo.update(&db).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "DB_ERROR".to_string(),
            }),
        )
    })?;

    Ok(Json(MemoResponse::from(memo)))
}

/// Delete a memo
#[vespera::route(delete, path = "/api/memos/{id}", tags = ["memo"])]
pub async fn delete_memo(
    Extension(db): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let result = Memo::delete_by_id(id).exec(&db).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
                code: "DB_ERROR".to_string(),
            }),
        )
    })?;

    if result.rows_affected == 0 {
        return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Memo not found".to_string(),
                code: "NOT_FOUND".to_string(),
            }),
        ));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted_id": id
    })))
}
