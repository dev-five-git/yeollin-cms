//! Administrator webhook configuration and delivery history APIs.

use std::fmt::Write;

use axum::{extract::Query, Extension, Json};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{Authorize, CurrentUser, EventBus, PluginError};

use crate::{
    delivery::{validate_url, MAX_ATTEMPTS, STATUS_DEAD_LETTER, STATUS_DELIVERED, STATUS_PENDING},
    models::{deliveries, endpoints},
};

const DEFAULT_PAGE_SIZE: u64 = 25;
const MAX_PAGE_SIZE: u64 = 100;
const ID_BYTES: usize = 16;
const MIN_SECRET_BYTES: usize = 32;
const MAX_SECRET_BYTES: usize = 512;
const MAX_NAME_CHARS: usize = 100;
const MAX_EVENT_NAMES: usize = 100;
const MAX_EVENT_NAME_CHARS: usize = 128;
const MAX_URL_CHARS: usize = 2_048;
const MIN_TIMEOUT_SECONDS: i32 = 1;
const MAX_TIMEOUT_SECONDS: i32 = 30;

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResponse {
    pub id: String,
    pub name: String,
    pub url: String,
    pub event_names: Vec<String>,
    pub allow_private_networks: bool,
    pub timeout_seconds: i32,
    pub enabled: bool,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<endpoints::Model> for WebhookResponse {
    type Error = PluginError;

    fn try_from(model: endpoints::Model) -> Result<Self, Self::Error> {
        let event_names = serde_json::from_value(model.event_names).map_err(|error| {
            tracing::error!(%error, webhook_id = %model.id, "Stored webhook filter is invalid");
            PluginError::internal()
        })?;
        Ok(Self {
            id: model.id,
            name: model.name,
            url: model.url,
            event_names,
            allow_private_networks: model.allow_private_networks,
            timeout_seconds: model.timeout_seconds,
            enabled: model.enabled,
            has_secret: !model.secret.is_empty(),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        })
    }
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListWebhooksResponse {
    pub webhooks: Vec<WebhookResponse>,
}

#[derive(Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub secret: String,
    #[serde(default)]
    #[schema(default = "[]")]
    pub event_names: Vec<String>,
    #[serde(default)]
    pub allow_private_networks: bool,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: i32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWebhookRequest {
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    #[serde(default)]
    #[schema(default = "[]")]
    pub event_names: Vec<String>,
    #[serde(default)]
    pub allow_private_networks: bool,
    pub timeout_seconds: i32,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWebhookResponse {
    pub success: bool,
    pub deleted_id: String,
}

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryResponse {
    pub id: String,
    pub webhook_id: String,
    pub event_id: i64,
    pub event_name: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub response_status: Option<i32>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub delivered_at: Option<String>,
}

impl From<deliveries::Model> for DeliveryResponse {
    fn from(model: deliveries::Model) -> Self {
        Self {
            id: model.id,
            webhook_id: model.webhook_id,
            event_id: model.event_id,
            event_name: model.event_name,
            status: model.status,
            attempts: model.attempts,
            max_attempts: MAX_ATTEMPTS,
            response_status: model.response_status,
            last_error: model.last_error,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
            delivered_at: model.delivered_at.map(|value| value.to_rfc3339()),
        }
    }
}

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListDeliveriesQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub webhook_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListDeliveriesResponse {
    pub deliveries: Vec<DeliveryResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// List configured endpoints without returning signing secrets.
#[vespera::route(get, tags = ["webhooks"])]
pub async fn list_webhooks(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<ListWebhooksResponse>, PluginError> {
    current.require_role("admin")?;
    let webhooks = endpoints::Entity::find()
        .order_by(endpoints::Column::CreatedAt, Order::Desc)
        .all(&db)
        .await?
        .into_iter()
        .map(WebhookResponse::try_from)
        .collect::<Result<_, _>>()?;
    Ok(Json(ListWebhooksResponse { webhooks }))
}

/// Create an endpoint with a write-only signing secret.
#[vespera::route(post, tags = ["webhooks"])]
pub async fn create_webhook(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Json(request): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookResponse>, PluginError> {
    current.require_role("admin")?;
    let values = validate_values(
        request.name,
        request.url,
        request.event_names,
        request.timeout_seconds,
    )?;
    let secret = validate_secret(request.secret)?;
    ensure_name_available(&db, &values.name, None).await?;
    let now = chrono::Utc::now();
    let model = endpoints::ActiveModel {
        id: Set(random_id()),
        name: Set(values.name),
        url: Set(values.url),
        secret: Set(secret),
        event_names: Set(serde_json::json!(values.event_names)),
        allow_private_networks: Set(request.allow_private_networks),
        timeout_seconds: Set(values.timeout_seconds),
        enabled: Set(request.enabled),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;
    Ok(Json(WebhookResponse::try_from(model)?))
}

/// Replace endpoint configuration; omit `secret` to retain the existing value.
#[vespera::route(put, path = "/{id}", tags = ["webhooks"])]
pub async fn update_webhook(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(request): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, PluginError> {
    current.require_role("admin")?;
    let Some(existing) = endpoints::Entity::find_by_id(&id).one(&db).await? else {
        return Err(PluginError::not_found("Webhook not found"));
    };
    let values = validate_values(
        request.name,
        request.url,
        request.event_names,
        request.timeout_seconds,
    )?;
    ensure_name_available(&db, &values.name, Some(&id)).await?;
    let secret = request.secret.map(validate_secret).transpose()?;
    let mut active: endpoints::ActiveModel = existing.into();
    active.name = Set(values.name);
    active.url = Set(values.url);
    if let Some(secret) = secret {
        active.secret = Set(secret);
    }
    active.event_names = Set(serde_json::json!(values.event_names));
    active.allow_private_networks = Set(request.allow_private_networks);
    active.timeout_seconds = Set(values.timeout_seconds);
    active.enabled = Set(request.enabled);
    active.updated_at = Set(chrono::Utc::now().into());
    let model = active.update(&db).await?;
    Ok(Json(WebhookResponse::try_from(model)?))
}

/// Delete an endpoint and its per-delivery history.
#[vespera::route(delete, path = "/{id}", tags = ["webhooks"])]
pub async fn delete_webhook(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<DeleteWebhookResponse>, PluginError> {
    current.require_role("admin")?;
    if endpoints::Entity::find_by_id(&id).one(&db).await?.is_none() {
        return Err(PluginError::not_found("Webhook not found"));
    }
    let transaction = db.begin().await?;
    deliveries::Entity::delete_many()
        .filter(deliveries::Column::WebhookId.eq(&id))
        .exec(&transaction)
        .await?;
    endpoints::Entity::delete_by_id(&id)
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    Ok(Json(DeleteWebhookResponse {
        success: true,
        deleted_id: id,
    }))
}

/// List recent endpoint deliveries and dead letters.
#[vespera::route(get, path = "/deliveries", tags = ["webhooks"])]
pub async fn list_deliveries(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<ListDeliveriesQuery>,
) -> Result<Json<ListDeliveriesResponse>, PluginError> {
    current.require_role("admin")?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let mut find = deliveries::Entity::find();
    if let Some(webhook_id) = query
        .webhook_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        find = find.filter(deliveries::Column::WebhookId.eq(webhook_id));
    }
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !matches!(
            status,
            STATUS_PENDING | STATUS_DELIVERED | STATUS_DEAD_LETTER
        ) {
            return Err(PluginError::bad_request("Unknown delivery status"));
        }
        find = find.filter(deliveries::Column::Status.eq(status));
    }
    let paginator = find
        .order_by(deliveries::Column::CreatedAt, Order::Desc)
        .paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let deliveries = paginator
        .fetch_page(page - 1)
        .await?
        .into_iter()
        .map(DeliveryResponse::from)
        .collect();
    Ok(Json(ListDeliveriesResponse {
        deliveries,
        total,
        page,
        page_size,
    }))
}

/// Reset a dead letter and immediately requeue its immutable source event.
#[vespera::route(post, path = "/deliveries/{id}/retry", tags = ["webhooks"])]
pub async fn retry_delivery(
    Extension(db): Extension<DatabaseConnection>,
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<DeliveryResponse>, PluginError> {
    current.require_role("admin")?;
    let Some(delivery) = deliveries::Entity::find_by_id(&id).one(&db).await? else {
        return Err(PluginError::not_found("Webhook delivery not found"));
    };
    if delivery.status != STATUS_DEAD_LETTER {
        return Err(PluginError::conflict(
            "Only dead-letter deliveries can be retried",
        ));
    }
    let Some(endpoint) = endpoints::Entity::find_by_id(&delivery.webhook_id)
        .one(&db)
        .await?
    else {
        return Err(PluginError::bad_request("The webhook no longer exists"));
    };
    if !endpoint.enabled {
        return Err(PluginError::bad_request(
            "Enable the webhook before retrying its delivery",
        ));
    }

    let event_id = delivery.event_id;
    let previous = delivery.clone();
    let mut active: deliveries::ActiveModel = delivery.into();
    active.status = Set(STATUS_PENDING.to_string());
    active.attempts = Set(0);
    active.response_status = Set(None);
    active.last_error = Set(None);
    active.updated_at = Set(chrono::Utc::now().into());
    active.delivered_at = Set(None);
    let reset = active.update(&db).await?;
    match events.requeue(event_id).await {
        Ok(true) => {}
        result => {
            let mut restore: deliveries::ActiveModel = reset.clone().into();
            restore.status = Set(previous.status);
            restore.attempts = Set(previous.attempts);
            restore.response_status = Set(previous.response_status);
            restore.last_error = Set(previous.last_error);
            restore.updated_at = Set(chrono::Utc::now().into());
            restore.delivered_at = Set(previous.delivered_at);
            restore.update(&db).await?;
            if let Err(error) = result {
                return Err(error.into());
            }
            return Err(PluginError::bad_request(
                "The source event is no longer available",
            ));
        }
    }
    Ok(Json(DeliveryResponse::from(reset)))
}

struct ValidatedValues {
    name: String,
    url: String,
    event_names: Vec<String>,
    timeout_seconds: i32,
}

fn validate_values(
    name: String,
    url: String,
    event_names: Vec<String>,
    timeout_seconds: i32,
) -> Result<ValidatedValues, PluginError> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(PluginError::bad_request(format!(
            "Name must contain 1 to {MAX_NAME_CHARS} characters"
        )));
    }
    if url.chars().count() > MAX_URL_CHARS {
        return Err(PluginError::bad_request("Webhook URL is too long"));
    }
    let url = validate_url(&url)
        .map_err(PluginError::bad_request)?
        .to_string();
    let event_names = normalize_event_names(event_names)?;
    if !(MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        return Err(PluginError::bad_request(format!(
            "timeoutSeconds must be between {MIN_TIMEOUT_SECONDS} and {MAX_TIMEOUT_SECONDS}"
        )));
    }
    Ok(ValidatedValues {
        name,
        url,
        event_names,
        timeout_seconds,
    })
}

fn normalize_event_names(names: Vec<String>) -> Result<Vec<String>, PluginError> {
    if names.len() > MAX_EVENT_NAMES {
        return Err(PluginError::bad_request(format!(
            "At most {MAX_EVENT_NAMES} event names are allowed"
        )));
    }
    let mut normalized = Vec::with_capacity(names.len());
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if name.chars().count() > MAX_EVENT_NAME_CHARS
            || !name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ':')
            })
        {
            return Err(PluginError::bad_request(format!(
                "Invalid event name `{name}`"
            )));
        }
        normalized.push(name.to_string());
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn validate_secret(secret: String) -> Result<String, PluginError> {
    let length = secret.len();
    if !(MIN_SECRET_BYTES..=MAX_SECRET_BYTES).contains(&length) {
        return Err(PluginError::bad_request(format!(
            "Secret must contain {MIN_SECRET_BYTES} to {MAX_SECRET_BYTES} bytes"
        )));
    }
    Ok(secret)
}

async fn ensure_name_available(
    db: &DatabaseConnection,
    name: &str,
    except_id: Option<&str>,
) -> Result<(), PluginError> {
    let mut find = endpoints::Entity::find().filter(endpoints::Column::Name.eq(name));
    if let Some(id) = except_id {
        find = find.filter(endpoints::Column::Id.ne(id));
    }
    if find.one(db).await?.is_some() {
        return Err(PluginError::conflict(
            "A webhook with this name already exists",
        ));
    }
    Ok(())
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

const fn default_timeout_seconds() -> i32 {
    5
}

const fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_are_exact_normalized_and_deduplicated() {
        assert_eq!(
            normalize_event_names(vec![
                " memo.updated ".to_string(),
                "memo.created".to_string(),
                "memo.updated".to_string(),
                String::new(),
            ])
            .unwrap(),
            ["memo.created", "memo.updated"]
        );
        assert!(normalize_event_names(vec!["memo created".to_string()]).is_err());
    }

    #[test]
    fn secrets_and_timeouts_are_bounded() {
        assert!(validate_secret("short".to_string()).is_err());
        assert!(validate_secret("s".repeat(MIN_SECRET_BYTES)).is_ok());
        assert!(validate_values(
            "Hook".to_string(),
            "https://example.com/hook".to_string(),
            Vec::new(),
            MAX_TIMEOUT_SECONDS + 1,
        )
        .is_err());
    }
}
