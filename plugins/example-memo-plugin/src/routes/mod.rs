//! Memo CRUD routes, mounted under the plugin API namespace.

use axum::{extract::Path, Extension, Json};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, Order, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{Event, EventBus, PluginError, PluginResult};

use crate::models::memo;

/// Memo response
#[derive(Clone, Serialize, Schema)]
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

/// Delete memo response
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMemoResponse {
    pub success: bool,
    pub deleted_id: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoCreated {
    memo: MemoResponse,
}

impl Event for MemoCreated {
    const NAME: &'static str = "memo.created";
    const AUDIT: bool = true;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoUpdated {
    memo: MemoResponse,
}

impl Event for MemoUpdated {
    const NAME: &'static str = "memo.updated";
    const AUDIT: bool = true;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoDeleted {
    id: i32,
}

impl Event for MemoDeleted {
    const NAME: &'static str = "memo.deleted";
    const AUDIT: bool = true;
}

async fn find_memo<C>(db: &C, id: i32) -> PluginResult<memo::Model>
where
    C: ConnectionTrait,
{
    memo::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| PluginError::not_found("Memo not found"))
}

/// List all memos
#[vespera::route(get, tags = ["memo"])]
pub async fn list_memos(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<ListMemosResponse>, PluginError> {
    let memos = memo::Entity::find()
        .order_by(memo::Column::CreatedAt, Order::Desc)
        .all(&db)
        .await?;

    let total = memos.len() as u64;
    let memos = memos.into_iter().map(MemoResponse::from).collect();

    Ok(Json(ListMemosResponse { memos, total }))
}

/// Get a memo by ID
#[vespera::route(get, path = "/{id}", tags = ["memo"])]
pub async fn get_memo(
    Extension(db): Extension<DatabaseConnection>,
    Path(id): Path<i32>,
) -> Result<Json<MemoResponse>, PluginError> {
    Ok(Json(MemoResponse::from(find_memo(&db, id).await?)))
}

/// Create a new memo
#[vespera::route(post, tags = ["memo"])]
pub async fn create_memo(
    Extension(events): Extension<EventBus>,
    Json(req): Json<CreateMemoRequest>,
) -> Result<Json<MemoResponse>, PluginError> {
    let mut transaction = events.begin().await?;
    let now = chrono::Utc::now();
    let memo = memo::ActiveModel {
        title: Set(req.title),
        content: Set(req.content),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(transaction.connection())
    .await?;
    let response = MemoResponse::from(memo);
    transaction
        .emit(&MemoCreated {
            memo: response.clone(),
        })
        .await?;
    transaction.commit().await?;

    Ok(Json(response))
}

/// Update a memo
#[vespera::route(patch, path = "/{id}", tags = ["memo"])]
pub async fn update_memo(
    Extension(events): Extension<EventBus>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateMemoRequest>,
) -> Result<Json<MemoResponse>, PluginError> {
    let mut transaction = events.begin().await?;
    let mut memo: memo::ActiveModel = find_memo(transaction.connection(), id).await?.into();

    if let Some(title) = req.title {
        memo.title = Set(title);
    }
    if let Some(content) = req.content {
        memo.content = Set(content);
    }
    memo.updated_at = Set(chrono::Utc::now().into());

    let response = MemoResponse::from(memo.update(transaction.connection()).await?);
    transaction
        .emit(&MemoUpdated {
            memo: response.clone(),
        })
        .await?;
    transaction.commit().await?;

    Ok(Json(response))
}

/// Delete a memo
#[vespera::route(delete, path = "/{id}", tags = ["memo"])]
pub async fn delete_memo(
    Extension(events): Extension<EventBus>,
    Path(id): Path<i32>,
) -> Result<Json<DeleteMemoResponse>, PluginError> {
    let mut transaction = events.begin().await?;
    let outcome = memo::Entity::delete_by_id(id)
        .exec(transaction.connection())
        .await?;

    if outcome.rows_affected == 0 {
        return Err(PluginError::not_found("Memo not found"));
    }

    transaction.emit(&MemoDeleted { id }).await?;
    transaction.commit().await?;

    Ok(Json(DeleteMemoResponse {
        success: true,
        deleted_id: id,
    }))
}
