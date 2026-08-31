//! Form definitions, public submission, and administrator inbox routes.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use axum::{extract::Query, Extension, Json};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, Order,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use vespera::Schema;
use yeollin_plugin::{Authorize, CurrentUser, Event, EventBus, PluginError, PluginResult};

use crate::models::{definition, submission};

const DEFAULT_PAGE_SIZE: u64 = 25;
const MAX_PAGE_SIZE: u64 = 100;
const ID_BYTES: usize = 16;
const MAX_FORM_NAME_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_SUCCESS_MESSAGE_CHARS: usize = 240;
const MAX_FIELDS: usize = 20;
const MAX_FIELD_ID_CHARS: usize = 64;
const MAX_FIELD_LABEL_CHARS: usize = 100;
const MAX_PLACEHOLDER_CHARS: usize = 160;
const MAX_OPTIONS: usize = 50;
const MAX_OPTION_CHARS: usize = 100;
const MAX_TEXT_VALUE_CHARS: usize = 500;
const MAX_TEXTAREA_VALUE_CHARS: usize = 5_000;
const DEFAULT_SUBMISSIONS_PER_HOUR: i32 = 100;
const MAX_SUBMISSIONS_PER_HOUR: i32 = 10_000;

/// A field that public clients render and the server validates on submission.
#[derive(Clone, Debug, Deserialize, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub id: String,
    pub label: String,
    pub kind: FormFieldKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    #[schema(default = "[]")]
    pub options: Vec<String>,
    pub placeholder: Option<String>,
}

/// Supported, intentionally small public field vocabulary.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Schema)]
#[serde(rename_all = "lowercase")]
pub enum FormFieldKind {
    Text,
    Email,
    Textarea,
    Checkbox,
    Select,
}

#[derive(Clone, Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFormRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<FormField>,
    #[serde(default = "default_success_message")]
    pub success_message: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_submissions_per_hour")]
    #[schema(default = 100)]
    pub max_submissions_per_hour: i32,
}

#[derive(Clone, Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFormRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<FormField>,
    pub success_message: String,
    pub enabled: bool,
    pub max_submissions_per_hour: i32,
}

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct FormResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<FormField>,
    pub success_message: String,
    pub enabled: bool,
    pub max_submissions_per_hour: i32,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListFormsResponse {
    pub forms: Vec<FormResponse>,
}

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct PublicFormQuery {
    pub id: String,
}

/// Public rendering contract. Rate limits and creator identity never leave the admin API.
#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct PublicFormResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fields: Vec<FormField>,
    pub success_message: String,
}

#[derive(Debug, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitFormRequest {
    pub form_id: String,
    pub values: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitFormResponse {
    pub submission_id: String,
    pub success_message: String,
}

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListSubmissionsQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct FormSubmissionResponse {
    pub id: String,
    pub form_id: String,
    pub form_name: String,
    pub fields: Vec<FormField>,
    pub values: Value,
    pub created_at: String,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListSubmissionsResponse {
    pub submissions: Vec<FormSubmissionResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFormResponse {
    pub success: bool,
    pub deleted_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FormChanged {
    actor: String,
    form_id: String,
    name: String,
    enabled: bool,
}

impl Event for FormChanged {
    const NAME: &'static str = "forms.changed";
    const AUDIT: bool = true;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FormDeleted {
    actor: String,
    form_id: String,
    name: String,
}

impl Event for FormDeleted {
    const NAME: &'static str = "forms.deleted";
    const AUDIT: bool = true;
}

/// Public submissions deliberately omit values from the event envelope so PII
/// cannot accidentally reach audit history or a generic webhook subscriber.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FormSubmitted {
    form_id: String,
    submission_id: String,
}

impl Event for FormSubmitted {
    const NAME: &'static str = "forms.submitted";
}

/// List every administrator-managed form.
#[vespera::route(get, tags = ["forms"])]
pub async fn list_forms(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<ListFormsResponse>, PluginError> {
    current.require_role("admin")?;
    let forms = definition::Entity::find()
        .order_by(definition::Column::CreatedAt, Order::Desc)
        .all(&db)
        .await?
        .into_iter()
        .map(FormResponse::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListFormsResponse { forms }))
}

/// Create a public form with server-enforced field definitions.
#[vespera::route(post, tags = ["forms"])]
pub async fn create_form(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    Json(request): Json<CreateFormRequest>,
) -> Result<Json<FormResponse>, PluginError> {
    current.require_role("admin")?;
    let values = validate_form_values(
        request.name,
        request.description,
        request.fields,
        request.success_message,
        request.enabled,
        request.max_submissions_per_hour,
    )?;
    let mut transaction = events.begin().await?;
    ensure_name_available(transaction.connection(), &values.name, None).await?;
    let now = chrono::Utc::now();
    let model = definition::ActiveModel {
        id: Set(random_id()),
        name: Set(values.name),
        description: Set(values.description),
        fields: Set(serde_json::to_value(values.fields).map_err(internal_serialize)?),
        success_message: Set(values.success_message),
        enabled: Set(values.enabled),
        max_submissions_per_hour: Set(values.max_submissions_per_hour),
        created_by: Set(current.sub.clone()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(transaction.connection())
    .await?;
    let response = FormResponse::try_from(model)?;
    transaction
        .emit(&FormChanged {
            actor: current.sub,
            form_id: response.id.clone(),
            name: response.name.clone(),
            enabled: response.enabled,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(response))
}

/// Fetch one form and its live public-field definition.
#[vespera::route(get, path = "/{id}", tags = ["forms"])]
pub async fn get_form(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<FormResponse>, PluginError> {
    current.require_role("admin")?;
    let form = find_form(&db, &id).await?;
    Ok(Json(FormResponse::try_from(form)?))
}

/// Replace a form definition while preserving submissions with their original field snapshots.
#[vespera::route(put, path = "/{id}", tags = ["forms"])]
pub async fn update_form(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(request): Json<UpdateFormRequest>,
) -> Result<Json<FormResponse>, PluginError> {
    current.require_role("admin")?;
    let id = canonical_id(&id).ok_or_else(|| PluginError::not_found("Form not found"))?;
    let values = validate_form_values(
        request.name,
        request.description,
        request.fields,
        request.success_message,
        request.enabled,
        request.max_submissions_per_hour,
    )?;
    let mut transaction = events.begin().await?;
    let Some(existing) = definition::Entity::find_by_id(&id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Form not found"));
    };
    ensure_name_available(transaction.connection(), &values.name, Some(&id)).await?;
    let mut active: definition::ActiveModel = existing.into();
    active.name = Set(values.name);
    active.description = Set(values.description);
    active.fields = Set(serde_json::to_value(values.fields).map_err(internal_serialize)?);
    active.success_message = Set(values.success_message);
    active.enabled = Set(values.enabled);
    active.max_submissions_per_hour = Set(values.max_submissions_per_hour);
    active.updated_at = Set(chrono::Utc::now().into());
    let model = active.update(transaction.connection()).await?;
    let response = FormResponse::try_from(model)?;
    transaction
        .emit(&FormChanged {
            actor: current.sub,
            form_id: response.id.clone(),
            name: response.name.clone(),
            enabled: response.enabled,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(response))
}

/// Delete a form and its submissions in the same transaction.
#[vespera::route(delete, path = "/{id}", tags = ["forms"])]
pub async fn delete_form(
    Extension(events): Extension<EventBus>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<DeleteFormResponse>, PluginError> {
    current.require_role("admin")?;
    let id = canonical_id(&id).ok_or_else(|| PluginError::not_found("Form not found"))?;
    let mut transaction = events.begin().await?;
    let Some(form) = definition::Entity::find_by_id(&id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Form not found"));
    };
    submission::Entity::delete_many()
        .filter(submission::Column::FormId.eq(&id))
        .exec(transaction.connection())
        .await?;
    definition::Entity::delete_by_id(&id)
        .exec(transaction.connection())
        .await?;
    transaction
        .emit(&FormDeleted {
            actor: current.sub,
            form_id: id.clone(),
            name: form.name,
        })
        .await?;
    transaction.commit().await?;
    Ok(Json(DeleteFormResponse {
        success: true,
        deleted_id: id,
    }))
}

/// Read the public rendering contract for one enabled form.
#[vespera::route(get, path = "/public", tags = ["forms"])]
pub async fn public_form(
    Extension(db): Extension<DatabaseConnection>,
    Query(query): Query<PublicFormQuery>,
) -> Result<Json<PublicFormResponse>, PluginError> {
    let form = find_form(&db, &query.id).await?;
    if !form.enabled {
        return Err(PluginError::not_found("Form not found"));
    }
    let fields = decode_fields(form.fields)?;
    Ok(Json(PublicFormResponse {
        id: form.id,
        name: form.name,
        description: form.description,
        fields,
        success_message: form.success_message,
    }))
}

/// Validate and store one public form submission.
#[vespera::route(post, path = "/submit", tags = ["forms"])]
pub async fn submit_form(
    Extension(events): Extension<EventBus>,
    Json(request): Json<SubmitFormRequest>,
) -> Result<Json<SubmitFormResponse>, PluginError> {
    let form_id =
        canonical_id(&request.form_id).ok_or_else(|| PluginError::not_found("Form not found"))?;
    let mut transaction = events.begin().await?;
    let Some(form) = definition::Entity::find_by_id(&form_id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Form not found"));
    };
    if !form.enabled {
        return Err(PluginError::not_found("Form not found"));
    }
    let fields = decode_fields(form.fields.clone())?;
    let values = validate_submission_values(&fields, request.values)?;
    let submitted_since = chrono::Utc::now() - chrono::Duration::hours(1);
    let submissions_this_hour = submission::Entity::find()
        .filter(submission::Column::FormId.eq(&form_id))
        .filter(submission::Column::CreatedAt.gte(submitted_since))
        .count(transaction.connection())
        .await?;
    if submissions_this_hour >= u64::try_from(form.max_submissions_per_hour).unwrap_or_default() {
        return Err(PluginError::too_many_requests(
            "This form has reached its submission limit. Please try again later.",
        ));
    }

    let submission_id = random_id();
    let now = chrono::Utc::now();
    submission::ActiveModel {
        id: Set(submission_id.clone()),
        form_id: Set(form_id.clone()),
        form_name: Set(form.name),
        form_fields: Set(form.fields),
        values: Set(values),
        created_at: Set(now.into()),
    }
    .insert(transaction.connection())
    .await?;
    transaction
        .emit(&FormSubmitted {
            form_id,
            submission_id: submission_id.clone(),
        })
        .await?;
    let success_message = form.success_message;
    transaction.commit().await?;
    Ok(Json(SubmitFormResponse {
        submission_id,
        success_message,
    }))
}

/// Read preserved submission snapshots for one form.
#[vespera::route(get, path = "/{id}/submissions", tags = ["forms"])]
pub async fn list_submissions(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Query(query): Query<ListSubmissionsQuery>,
) -> Result<Json<ListSubmissionsResponse>, PluginError> {
    current.require_role("admin")?;
    let form = find_form(&db, &id).await?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let paginator = submission::Entity::find()
        .filter(submission::Column::FormId.eq(form.id))
        .order_by(submission::Column::CreatedAt, Order::Desc)
        .paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let submissions = paginator
        .fetch_page(page - 1)
        .await?
        .into_iter()
        .map(FormSubmissionResponse::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListSubmissionsResponse {
        submissions,
        total,
        page,
        page_size,
    }))
}

impl TryFrom<definition::Model> for FormResponse {
    type Error = PluginError;

    fn try_from(model: definition::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            name: model.name,
            description: model.description,
            fields: decode_fields(model.fields)?,
            success_message: model.success_message,
            enabled: model.enabled,
            max_submissions_per_hour: model.max_submissions_per_hour,
            created_by: model.created_by,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        })
    }
}

impl TryFrom<submission::Model> for FormSubmissionResponse {
    type Error = PluginError;

    fn try_from(model: submission::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            form_id: model.form_id,
            form_name: model.form_name,
            fields: decode_fields(model.form_fields)?,
            values: model.values,
            created_at: model.created_at.to_rfc3339(),
        })
    }
}

struct ValidatedFormValues {
    name: String,
    description: String,
    fields: Vec<FormField>,
    success_message: String,
    enabled: bool,
    max_submissions_per_hour: i32,
}

fn validate_form_values(
    name: String,
    description: String,
    fields: Vec<FormField>,
    success_message: String,
    enabled: bool,
    max_submissions_per_hour: i32,
) -> PluginResult<ValidatedFormValues> {
    let name = bounded_text(name, "Name", 1, MAX_FORM_NAME_CHARS)?;
    let description = bounded_text(description, "Description", 0, MAX_DESCRIPTION_CHARS)?;
    let success_message = bounded_text(
        success_message,
        "Success message",
        1,
        MAX_SUCCESS_MESSAGE_CHARS,
    )?;
    if !(1..=MAX_FIELDS).contains(&fields.len()) {
        return Err(PluginError::bad_request(format!(
            "A form must contain 1 to {MAX_FIELDS} fields",
        )));
    }
    if !(1..=MAX_SUBMISSIONS_PER_HOUR).contains(&max_submissions_per_hour) {
        return Err(PluginError::bad_request(format!(
            "maxSubmissionsPerHour must be between 1 and {MAX_SUBMISSIONS_PER_HOUR}",
        )));
    }

    let mut field_ids = HashSet::new();
    let fields = fields
        .into_iter()
        .map(|field| validate_field(field, &mut field_ids))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValidatedFormValues {
        name,
        description,
        fields,
        success_message,
        enabled,
        max_submissions_per_hour,
    })
}

fn validate_field(field: FormField, field_ids: &mut HashSet<String>) -> PluginResult<FormField> {
    let id = field.id.trim().to_string();
    if !is_field_id(&id) {
        return Err(PluginError::bad_request(format!(
            "Field id `{id}` must be lowercase kebab-case",
        )));
    }
    if !field_ids.insert(id.clone()) {
        return Err(PluginError::bad_request(format!(
            "Field id `{id}` is duplicated",
        )));
    }
    let label = bounded_text(field.label, "Field label", 1, MAX_FIELD_LABEL_CHARS)?;
    let placeholder = field
        .placeholder
        .map(|value| bounded_text(value, "Placeholder", 0, MAX_PLACEHOLDER_CHARS))
        .transpose()?
        .filter(|value| !value.is_empty());
    let mut option_values = HashSet::new();
    let options = field
        .options
        .into_iter()
        .map(|option| bounded_text(option, "Select option", 1, MAX_OPTION_CHARS))
        .collect::<Result<Vec<_>, _>>()?;
    if options.len() > MAX_OPTIONS {
        return Err(PluginError::bad_request(format!(
            "A select field may contain at most {MAX_OPTIONS} options",
        )));
    }
    for option in &options {
        if !option_values.insert(option.clone()) {
            return Err(PluginError::bad_request(
                "Select options must be unique after trimming",
            ));
        }
    }
    if matches!(field.kind, FormFieldKind::Select) {
        if options.is_empty() {
            return Err(PluginError::bad_request(
                "A select field needs at least one option",
            ));
        }
    } else if !options.is_empty() {
        return Err(PluginError::bad_request(
            "Only select fields may define options",
        ));
    }
    Ok(FormField {
        id,
        label,
        kind: field.kind,
        required: field.required,
        options,
        placeholder,
    })
}

fn validate_submission_values(
    fields: &[FormField],
    values: BTreeMap<String, Value>,
) -> PluginResult<Value> {
    let field_ids = fields
        .iter()
        .map(|field| field.id.as_str())
        .collect::<HashSet<_>>();
    if let Some(unknown) = values.keys().find(|id| !field_ids.contains(id.as_str())) {
        return Err(PluginError::bad_request(format!(
            "Unknown form field `{unknown}`",
        )));
    }

    let mut normalized = Map::new();
    for field in fields {
        let raw = values.get(&field.id);
        match field.kind {
            FormFieldKind::Text | FormFieldKind::Email | FormFieldKind::Textarea => {
                let Some(Value::String(value)) = raw else {
                    if field.required {
                        return Err(required_field_error(field));
                    }
                    if raw.is_some() {
                        return Err(string_field_error(field));
                    }
                    continue;
                };
                let value = value.trim();
                if value.is_empty() {
                    if field.required {
                        return Err(required_field_error(field));
                    }
                    continue;
                }
                let max = if matches!(field.kind, FormFieldKind::Textarea) {
                    MAX_TEXTAREA_VALUE_CHARS
                } else {
                    MAX_TEXT_VALUE_CHARS
                };
                if value.chars().count() > max {
                    return Err(PluginError::bad_request(format!(
                        "{} must be at most {max} characters",
                        field.label
                    )));
                }
                if matches!(field.kind, FormFieldKind::Email) && !is_email(value) {
                    return Err(PluginError::bad_request(format!(
                        "{} must be a valid email address",
                        field.label
                    )));
                }
                normalized.insert(field.id.clone(), Value::String(value.to_string()));
            }
            FormFieldKind::Checkbox => {
                let checked = match raw {
                    None => false,
                    Some(Value::Bool(value)) => *value,
                    Some(_) => {
                        return Err(PluginError::bad_request(format!(
                            "{} must be a true or false value",
                            field.label
                        )));
                    }
                };
                if field.required && !checked {
                    return Err(required_field_error(field));
                }
                normalized.insert(field.id.clone(), Value::Bool(checked));
            }
            FormFieldKind::Select => {
                let Some(Value::String(value)) = raw else {
                    if field.required {
                        return Err(required_field_error(field));
                    }
                    if raw.is_some() {
                        return Err(string_field_error(field));
                    }
                    continue;
                };
                let value = value.trim();
                if value.is_empty() && !field.required {
                    continue;
                }
                if !field.options.iter().any(|option| option == value) {
                    return Err(PluginError::bad_request(format!(
                        "{} has an invalid selected option",
                        field.label
                    )));
                }
                normalized.insert(field.id.clone(), Value::String(value.to_string()));
            }
        }
    }
    Ok(Value::Object(normalized))
}

async fn ensure_name_available<C>(
    connection: &C,
    name: &str,
    except_id: Option<&str>,
) -> PluginResult<()>
where
    C: ConnectionTrait,
{
    let mut find = definition::Entity::find().filter(definition::Column::Name.eq(name));
    if let Some(id) = except_id {
        find = find.filter(definition::Column::Id.ne(id));
    }
    if find.one(connection).await?.is_some() {
        return Err(PluginError::conflict(
            "A form with that name already exists",
        ));
    }
    Ok(())
}

async fn find_form(db: &DatabaseConnection, id: &str) -> PluginResult<definition::Model> {
    let id = canonical_id(id).ok_or_else(|| PluginError::not_found("Form not found"))?;
    definition::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| PluginError::not_found("Form not found"))
}

fn decode_fields(value: Value) -> PluginResult<Vec<FormField>> {
    serde_json::from_value(value).map_err(|error| {
        tracing::error!(%error, "Stored form field definition is invalid");
        PluginError::internal()
    })
}

fn internal_serialize(error: serde_json::Error) -> PluginError {
    tracing::error!(%error, "Could not serialize validated form fields");
    PluginError::internal()
}

fn bounded_text(value: String, label: &str, min: usize, max: usize) -> PluginResult<String> {
    let value = value.trim().to_string();
    let length = value.chars().count();
    if !(min..=max).contains(&length) {
        return Err(PluginError::bad_request(format!(
            "{label} must contain {min} to {max} characters",
        )));
    }
    Ok(value)
}

fn is_field_id(value: &str) -> bool {
    value.chars().count() <= MAX_FIELD_ID_CHARS
        && !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn canonical_id(value: &str) -> Option<String> {
    let value = value.trim();
    (value.len() == ID_BYTES * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
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

fn required_field_error(field: &FormField) -> PluginError {
    PluginError::bad_request(format!("{} is required", field.label))
}

fn string_field_error(field: &FormField) -> PluginError {
    PluginError::bad_request(format!("{} must be a text value", field.label))
}

fn is_email(value: &str) -> bool {
    let Some((local, domain)) = value.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.contains('.')
        && value.chars().count() <= 320
        && value.chars().all(|character| !character.is_whitespace())
}

fn default_success_message() -> String {
    "Thanks - we received your response.".to_string()
}

const fn default_enabled() -> bool {
    true
}

const fn default_submissions_per_hour() -> i32 {
    DEFAULT_SUBMISSIONS_PER_HOUR
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_field(id: &str, required: bool) -> FormField {
        FormField {
            id: id.to_string(),
            label: "Your name".to_string(),
            kind: FormFieldKind::Text,
            required,
            options: vec![],
            placeholder: None,
        }
    }

    #[test]
    fn field_definitions_are_bounded_typed_and_canonical() {
        assert!(validate_form_values(
            "Contact".to_string(),
            String::new(),
            vec![text_field("your-name", true)],
            default_success_message(),
            true,
            100,
        )
        .is_ok());
        assert!(validate_form_values(
            "Contact".to_string(),
            String::new(),
            vec![text_field("Not canonical", true)],
            default_success_message(),
            true,
            100,
        )
        .is_err());
        let mut text_with_options = text_field("name", true);
        text_with_options.options.push("unexpected".to_string());
        assert!(validate_form_values(
            "Contact".to_string(),
            String::new(),
            vec![text_with_options],
            default_success_message(),
            true,
            100,
        )
        .is_err());
    }

    #[test]
    fn submission_values_reject_unknown_fields_and_validate_required_values() {
        let fields = vec![
            text_field("name", true),
            FormField {
                id: "terms".to_string(),
                label: "Terms".to_string(),
                kind: FormFieldKind::Checkbox,
                required: true,
                options: vec![],
                placeholder: None,
            },
        ];
        assert!(validate_submission_values(&fields, BTreeMap::new()).is_err());
        let unknown = BTreeMap::from([("unknown".to_string(), Value::String("x".to_string()))]);
        assert!(validate_submission_values(&fields, unknown).is_err());
        let valid = BTreeMap::from([
            ("name".to_string(), Value::String(" Ada ".to_string())),
            ("terms".to_string(), Value::Bool(true)),
        ]);
        assert_eq!(
            validate_submission_values(&fields, valid).unwrap(),
            serde_json::json!({ "name": "Ada", "terms": true })
        );
    }

    #[test]
    fn public_ids_are_opaque_lowercase_hex() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(canonical_id(id), Some(id.to_string()));
        for invalid in [
            "",
            "0123",
            "0123456789ABCDEF0123456789ABCDEF",
            "../../etc/passwd",
        ] {
            assert_eq!(canonical_id(invalid), None, "accepted `{invalid}`");
        }
    }
}
