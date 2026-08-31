//! Redirect rule administration and the runtime lookup used by the app layer.

use std::fmt::Write;

use axum::{extract::Path, Extension, Json};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, Order,
    QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{
    Authorize, CurrentUser, Event, EventBus, PluginError, PluginResult, RedirectTarget,
};

use crate::models::rule;

const ID_BYTES: usize = 16;
const MAX_PATH_CHARS: usize = 2_048;

#[derive(Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRedirectRequest {
    pub source_path: String,
    pub destination_path: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRedirectRequest {
    pub source_path: String,
    pub destination_path: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct RedirectResponse {
    pub id: String,
    pub source_path: String,
    pub destination_path: String,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListRedirectsResponse {
    pub redirects: Vec<RedirectResponse>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRedirectResponse {
    pub success: bool,
    pub deleted_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedirectChanged {
    actor: String,
    redirect_id: String,
    source_path: String,
    destination_path: String,
    enabled: bool,
}

impl Event for RedirectChanged {
    const NAME: &'static str = "redirects.changed";
    const AUDIT: bool = true;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedirectDeleted {
    actor: String,
    redirect_id: String,
    source_path: String,
}

impl Event for RedirectDeleted {
    const NAME: &'static str = "redirects.deleted";
    const AUDIT: bool = true;
}

/// List all administrator-managed redirect rules.
#[vespera::route(get, tags = ["redirects"])]
pub async fn list_redirects(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<ListRedirectsResponse>, PluginError> {
    current.require_role("admin")?;
    let redirects = rule::Entity::find()
        .order_by(rule::Column::SourcePath, Order::Asc)
        .all(&db)
        .await?
        .into_iter()
        .map(RedirectResponse::from)
        .collect();
    Ok(Json(ListRedirectsResponse { redirects }))
}

/// Create a permanent redirect rule.
#[vespera::route(post, tags = ["redirects"])]
pub async fn create_redirect(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    Json(request): Json<CreateRedirectRequest>,
) -> Result<Json<RedirectResponse>, PluginError> {
    current.require_role("admin")?;
    let values = validate_rule(
        request.source_path,
        request.destination_path,
        request.enabled,
    )?;
    let mut transaction = events.begin().await?;
    ensure_source_available(transaction.connection(), &values.source_path, None).await?;
    let now = chrono::Utc::now();
    let model = rule::ActiveModel {
        id: Set(random_id()),
        source_path: Set(values.source_path),
        destination_path: Set(values.destination_path),
        enabled: Set(values.enabled),
        created_by: Set(current.sub.clone()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(transaction.connection())
    .await?;
    let response = RedirectResponse::from(model);
    transaction
        .emit(&RedirectChanged {
            actor: current.sub,
            redirect_id: response.id.clone(),
            source_path: response.source_path.clone(),
            destination_path: response.destination_path.clone(),
            enabled: response.enabled,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(response))
}

/// Read a redirect rule by its opaque identifier.
#[vespera::route(get, path = "/{id}", tags = ["redirects"])]
pub async fn get_redirect(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<RedirectResponse>, PluginError> {
    current.require_role("admin")?;
    Ok(Json(RedirectResponse::from(find_rule(&db, &id).await?)))
}

/// Replace a redirect rule while preserving its administrator ownership.
#[vespera::route(put, path = "/{id}", tags = ["redirects"])]
pub async fn update_redirect(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<String>,
    Json(request): Json<UpdateRedirectRequest>,
) -> Result<Json<RedirectResponse>, PluginError> {
    current.require_role("admin")?;
    let id = canonical_id(&id).ok_or_else(|| PluginError::not_found("Redirect not found"))?;
    let values = validate_rule(
        request.source_path,
        request.destination_path,
        request.enabled,
    )?;
    let mut transaction = events.begin().await?;
    let Some(existing) = rule::Entity::find_by_id(&id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Redirect not found"));
    };
    ensure_source_available(transaction.connection(), &values.source_path, Some(&id)).await?;
    let mut active: rule::ActiveModel = existing.into();
    active.source_path = Set(values.source_path);
    active.destination_path = Set(values.destination_path);
    active.enabled = Set(values.enabled);
    active.updated_at = Set(chrono::Utc::now().into());
    let response = RedirectResponse::from(active.update(transaction.connection()).await?);
    transaction
        .emit(&RedirectChanged {
            actor: current.sub,
            redirect_id: response.id.clone(),
            source_path: response.source_path.clone(),
            destination_path: response.destination_path.clone(),
            enabled: response.enabled,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(response))
}

/// Remove a redirect rule.
#[vespera::route(delete, path = "/{id}", tags = ["redirects"])]
pub async fn delete_redirect(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<String>,
) -> Result<Json<DeleteRedirectResponse>, PluginError> {
    current.require_role("admin")?;
    let id = canonical_id(&id).ok_or_else(|| PluginError::not_found("Redirect not found"))?;
    let mut transaction = events.begin().await?;
    let Some(existing) = rule::Entity::find_by_id(&id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Redirect not found"));
    };
    rule::Entity::delete_by_id(&id)
        .exec(transaction.connection())
        .await?;
    transaction
        .emit(&RedirectDeleted {
            actor: current.sub,
            redirect_id: id.clone(),
            source_path: existing.source_path,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(DeleteRedirectResponse {
        success: true,
        deleted_id: id,
    }))
}

/// Resolve an enabled rule for the framework's outer redirect middleware.
pub async fn resolve_redirect(
    db: DatabaseConnection,
    path: String,
) -> anyhow::Result<Option<RedirectTarget>> {
    let rule = rule::Entity::find()
        .filter(rule::Column::SourcePath.eq(path))
        .filter(rule::Column::Enabled.eq(true))
        .one(&db)
        .await?;
    Ok(rule.map(|rule| RedirectTarget::permanent(rule.destination_path)))
}

impl From<rule::Model> for RedirectResponse {
    fn from(model: rule::Model) -> Self {
        Self {
            id: model.id,
            source_path: model.source_path,
            destination_path: model.destination_path,
            enabled: model.enabled,
            created_by: model.created_by,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

struct ValidatedRule {
    source_path: String,
    destination_path: String,
    enabled: bool,
}

fn validate_rule(
    source_path: String,
    destination_path: String,
    enabled: bool,
) -> PluginResult<ValidatedRule> {
    let source_path = canonical_internal_path(source_path, "Source path")?;
    if source_path == "/" {
        return Err(PluginError::bad_request(
            "Source path cannot redirect the whole site",
        ));
    }
    if is_reserved_source(&source_path) {
        return Err(PluginError::bad_request(
            "Source path cannot replace an API, health, or asset endpoint",
        ));
    }
    let destination_path = canonical_destination(destination_path)?;
    if source_path == destination_path {
        return Err(PluginError::bad_request(
            "Source and destination paths must differ",
        ));
    }
    Ok(ValidatedRule {
        source_path,
        destination_path,
        enabled,
    })
}

fn canonical_destination(value: String) -> PluginResult<String> {
    if value.trim() != value {
        return Err(PluginError::bad_request(
            "Destination must not have surrounding whitespace",
        ));
    }
    if value.starts_with('/') {
        return canonical_internal_path(value, "Destination path");
    }
    if !value.starts_with("https://")
        || value.len() > MAX_PATH_CHARS
        || contains_forbidden_characters(&value)
        || value.contains('\\')
    {
        return Err(PluginError::bad_request(
            "Destination must be an internal path or an https URL",
        ));
    }
    let authority = value["https://".len()..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() || authority.contains('@') || authority.starts_with('.') {
        return Err(PluginError::bad_request(
            "Destination URL must include a valid https host",
        ));
    }
    Ok(value)
}

fn canonical_internal_path(value: String, label: &str) -> PluginResult<String> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_PATH_CHARS
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains("//")
        || value.contains('\\')
        || value.contains("..")
        || value.contains(['?', '#'])
        || contains_forbidden_characters(&value)
        || (value.len() > 1 && value.ends_with('/'))
    {
        return Err(PluginError::bad_request(format!(
            "{label} must be a canonical root-relative path",
        )));
    }
    Ok(value)
}

fn contains_forbidden_characters(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
}

fn is_reserved_source(path: &str) -> bool {
    matches!(path, "/api" | "/health" | "/favicon.ico")
        || [
            "/api/",
            "/_next/",
            "/static/",
            "/@",
            "/__vite_hmr",
            "/node_modules/",
            "/src/",
            "/df/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

async fn ensure_source_available(
    connection: &impl ConnectionTrait,
    source_path: &str,
    excluding_id: Option<&str>,
) -> PluginResult<()> {
    let mut find = rule::Entity::find().filter(rule::Column::SourcePath.eq(source_path));
    if let Some(id) = excluding_id {
        find = find.filter(rule::Column::Id.ne(id));
    }
    if find.one(connection).await?.is_some() {
        return Err(PluginError::conflict(
            "A redirect for that source path already exists",
        ));
    }
    Ok(())
}

async fn find_rule(db: &DatabaseConnection, id: &str) -> PluginResult<rule::Model> {
    let id = canonical_id(id).ok_or_else(|| PluginError::not_found("Redirect not found"))?;
    rule::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| PluginError::not_found("Redirect not found"))
}

fn canonical_id(value: &str) -> Option<String> {
    (value.len() == ID_BYTES * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
    .then(|| value.to_string())
}

fn random_id() -> String {
    rand::random::<[u8; ID_BYTES]>().iter().fold(
        String::with_capacity(ID_BYTES * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_paths_are_canonical_and_keep_system_routes_reserved() {
        for value in ["old", "//old", "/old/", "/old?x=1", "/old#x", "/../old"] {
            assert!(canonical_internal_path(value.to_string(), "Path").is_err());
        }
        assert!(is_reserved_source("/api"));
        assert!(is_reserved_source("/api/users"));
        assert!(is_reserved_source("/_next/static/app.js"));
        assert!(canonical_internal_path(" /old ".to_string(), "Path").is_err());
    }

    #[test]
    fn destinations_allow_internal_paths_or_safe_https_urls() {
        assert_eq!(canonical_destination("/new".to_string()).unwrap(), "/new");
        assert_eq!(
            canonical_destination("https://example.com/new?from=legacy".to_string()).unwrap(),
            "https://example.com/new?from=legacy"
        );
        for value in [
            "http://example.com",
            "https://",
            "https://@example.com",
            "/new path",
            " /new",
        ] {
            assert!(canonical_destination(value.to_string()).is_err());
        }
    }

    #[test]
    fn opaque_ids_are_strictly_lowercase_hex() {
        assert!(canonical_id("0123456789abcdef0123456789abcdef").is_some());
        assert!(canonical_id("0123456789abcdef0123456789abcdeg").is_none());
        assert!(canonical_id("0123456789ABCDEF0123456789ABCDEF").is_none());
    }
}
