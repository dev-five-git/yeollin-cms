//! Media library routes mounted at `/api/media`.

use std::io::ErrorKind;

use axum::{
    body::Body,
    extract::Query,
    http::{header, HeaderValue, Response, StatusCode},
    Extension, Json,
};
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;
use vespera::{
    multipart::{FieldData, TypedMultipart},
    Schema,
};
use yeollin_plugin::{
    Authorize, CurrentUser, Event, EventBus, PluginError, PluginResult, RuntimeStorage,
    SettingsStore, StorageError,
};

use crate::{models::asset, MediaSettings, HARD_UPLOAD_BYTES, PLUGIN_NAME};

const DEFAULT_PAGE_SIZE: u64 = 24;
const MAX_PAGE_SIZE: u64 = 100;
const REFERENCE_PREFIX: &str = "media:";
const ID_BYTES: usize = 16;

/// One uploaded media object.
#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MediaResponse {
    pub id: String,
    pub reference: String,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: String,
    pub created_at: String,
    pub url: String,
}

impl From<asset::Model> for MediaResponse {
    fn from(model: asset::Model) -> Self {
        let url = format!("/api/media/file?reference={}", model.reference);
        Self {
            id: model.id,
            reference: model.reference,
            original_name: model.original_name,
            mime_type: model.mime_type,
            size_bytes: model.size_bytes,
            uploaded_by: model.uploaded_by,
            created_at: model.created_at.to_rfc3339(),
            url,
        }
    }
}

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListMediaQuery {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListMediaResponse {
    pub media: Vec<MediaResponse>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// Multipart upload contract. A hard cap protects the temporary file before
/// the administrator-configured (possibly lower) cap is checked.
#[derive(vespera::Multipart, Schema)]
#[try_from_multipart(strict)]
pub struct UploadMediaRequest {
    #[form_data(limit = "10MiB")]
    pub file: FieldData<vespera::tempfile::NamedTempFile>,
}

#[derive(Debug, Deserialize, Schema)]
pub struct MediaFileQuery {
    pub reference: String,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMediaResponse {
    pub success: bool,
    pub deleted_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaUploaded {
    media: MediaResponse,
}

impl Event for MediaUploaded {
    const NAME: &'static str = "media.uploaded";
    const AUDIT: bool = true;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaDeleted {
    id: String,
    reference: String,
    original_name: String,
}

impl Event for MediaDeleted {
    const NAME: &'static str = "media.deleted";
    const AUDIT: bool = true;
}

/// List media newest first.
#[vespera::route(get, tags = ["media"])]
pub async fn list_media(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<ListMediaQuery>,
) -> Result<Json<ListMediaResponse>, PluginError> {
    current.require_role("admin")?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let paginator = asset::Entity::find()
        .order_by(asset::Column::CreatedAt, Order::Desc)
        .paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let media = paginator
        .fetch_page(page - 1)
        .await?
        .into_iter()
        .map(MediaResponse::from)
        .collect();

    Ok(Json(ListMediaResponse {
        media,
        total,
        page,
        page_size,
    }))
}

/// Upload one image into the runtime object store.
#[vespera::route(post, tags = ["media"])]
pub async fn upload_media(
    Extension(events): Extension<EventBus>,
    Extension(storage): Extension<RuntimeStorage>,
    Extension(settings): Extension<SettingsStore>,
    Extension(current): Extension<CurrentUser>,
    TypedMultipart(upload): TypedMultipart<UploadMediaRequest>,
) -> Result<Json<MediaResponse>, PluginError> {
    current.require_role("admin")?;
    let settings = settings.get::<MediaSettings>(PLUGIN_NAME).await?;
    let file_path = upload.file.contents.path();
    let metadata = tokio::fs::metadata(file_path).await.map_err(|error| {
        tracing::error!(%error, "Could not inspect multipart temporary file");
        PluginError::internal()
    })?;
    let size_bytes = metadata.len();
    let configured_limit = u64::from(settings.max_upload_megabytes) * 1024 * 1024;
    if size_bytes == 0 {
        return Err(PluginError::bad_request("The uploaded file is empty"));
    }
    if size_bytes > configured_limit {
        return Err(PluginError::payload_too_large(format!(
            "The file exceeds the configured {} MiB limit",
            settings.max_upload_megabytes
        )));
    }
    if size_bytes > HARD_UPLOAD_BYTES as u64 {
        return Err(PluginError::payload_too_large(
            "The file exceeds the media hard limit",
        ));
    }

    let mime_type = detect_file_mime(file_path).await?;
    if let Some(declared) = upload.file.metadata.content_type.as_deref() {
        let declared = declared.trim().to_ascii_lowercase();
        if declared != "application/octet-stream" && declared != mime_type {
            return Err(PluginError::unsupported_media_type(format!(
                "Declared MIME type `{declared}` does not match `{mime_type}`"
            )));
        }
    }
    let original_name = safe_original_name(upload.file.metadata.file_name.as_deref());
    let (id, reference) = store_with_new_id(&storage, file_path).await?;
    let stored_size = i64::try_from(size_bytes).map_err(|error| {
        tracing::error!(%error, size_bytes, "Media size does not fit the database type");
        PluginError::internal()
    })?;
    let now = chrono::Utc::now();

    let write: PluginResult<MediaResponse> = async {
        let mut transaction = events.begin().await?;
        let model = asset::ActiveModel {
            id: Set(id.clone()),
            reference: Set(reference.clone()),
            original_name: Set(original_name),
            mime_type: Set(mime_type.to_string()),
            size_bytes: Set(stored_size),
            uploaded_by: Set(current.sub),
            created_at: Set(now.into()),
        }
        .insert(transaction.connection())
        .await?;
        let response = MediaResponse::from(model);
        transaction
            .emit(&MediaUploaded {
                media: response.clone(),
            })
            .await?;
        transaction.commit().await?;
        Ok(response)
    }
    .await;

    match write {
        Ok(response) => Ok(Json(response)),
        Err(error) => {
            if let Err(cleanup_error) = storage.remove_file(PLUGIN_NAME, &id).await {
                tracing::warn!(%cleanup_error, %id, "Could not clean up media after a failed database write");
            }
            Err(error)
        }
    }
}

/// Serve one immutable media object by its stable reference.
#[vespera::route(get, path = "/file", tags = ["media"])]
pub async fn serve_media(
    Extension(db): Extension<DatabaseConnection>,
    Extension(storage): Extension<RuntimeStorage>,
    Query(query): Query<MediaFileQuery>,
) -> Result<Response<Body>, PluginError> {
    let id = parse_reference(&query.reference)
        .ok_or_else(|| PluginError::not_found("Media not found"))?;
    let Some(model) = asset::Entity::find_by_id(id).one(&db).await? else {
        return Err(PluginError::not_found("Media not found"));
    };

    let file = match storage.open_file(PLUGIN_NAME, &model.id).await {
        Ok(file) => file,
        Err(StorageError::Io(error)) if error.kind() == ErrorKind::NotFound => {
            tracing::error!(id = %model.id, "Media metadata points to a missing runtime object");
            return Err(PluginError::internal());
        }
        Err(error) => return Err(error.into()),
    };
    let content_type = HeaderValue::from_str(&model.mime_type).map_err(|error| {
        tracing::error!(%error, mime_type = %model.mime_type, "Stored media MIME type is invalid");
        PluginError::internal()
    })?;
    let etag = HeaderValue::from_str(&format!("\"{}\"", model.id)).map_err(|error| {
        tracing::error!(%error, id = %model.id, "Stored media ID cannot form an ETag");
        PluginError::internal()
    })?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&model.size_bytes.to_string()).map_err(|error| {
            tracing::error!(%error, "Stored media size cannot form a header");
            PluginError::internal()
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(header::ETAG, etag);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Delete media metadata and then reclaim its runtime object.
#[vespera::route(delete, path = "/{id}", tags = ["media"])]
pub async fn delete_media(
    Extension(events): Extension<EventBus>,
    Extension(storage): Extension<RuntimeStorage>,
    Extension(current): Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<DeleteMediaResponse>, PluginError> {
    current.require_role("admin")?;
    if !is_media_id(&id) {
        return Err(PluginError::not_found("Media not found"));
    }

    let mut transaction = events.begin().await?;
    let Some(model) = asset::Entity::find_by_id(&id)
        .one(transaction.connection())
        .await?
    else {
        return Err(PluginError::not_found("Media not found"));
    };
    asset::Entity::delete_by_id(&id)
        .exec(transaction.connection())
        .await?;
    transaction
        .emit(&MediaDeleted {
            id: model.id.clone(),
            reference: model.reference,
            original_name: model.original_name,
        })
        .await?;
    transaction.commit().await?;

    match storage.remove_file(PLUGIN_NAME, &id).await {
        Ok(true) => {}
        Ok(false) => tracing::warn!(%id, "Deleted media metadata had no runtime object"),
        Err(error) => tracing::warn!(%error, %id, "Could not reclaim deleted media object"),
    }

    Ok(Json(DeleteMediaResponse {
        success: true,
        deleted_id: id,
    }))
}

async fn store_with_new_id(
    storage: &RuntimeStorage,
    source: &std::path::Path,
) -> PluginResult<(String, String)> {
    for _ in 0..3 {
        let id = random_id();
        match storage.store_file(PLUGIN_NAME, &id, source).await {
            Ok(_) => return Ok((id.clone(), format!("{REFERENCE_PREFIX}{id}"))),
            Err(StorageError::AlreadyExists(_)) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    tracing::error!("Could not mint a unique media object ID after three attempts");
    Err(PluginError::internal())
}

fn random_id() -> String {
    use std::fmt::Write;
    rand::random::<[u8; ID_BYTES]>().iter().fold(
        String::with_capacity(ID_BYTES * 2),
        |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        },
    )
}

fn parse_reference(reference: &str) -> Option<&str> {
    let id = reference.strip_prefix(REFERENCE_PREFIX)?;
    is_media_id(id).then_some(id)
}

fn is_media_id(id: &str) -> bool {
    id.len() == ID_BYTES * 2
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

async fn detect_file_mime(path: &std::path::Path) -> PluginResult<&'static str> {
    let mut file = tokio::fs::File::open(path).await.map_err(|error| {
        tracing::error!(%error, "Could not open multipart temporary file");
        PluginError::internal()
    })?;
    let mut signature = [0_u8; 16];
    let read = file.read(&mut signature).await.map_err(|error| {
        tracing::error!(%error, "Could not inspect multipart file signature");
        PluginError::internal()
    })?;
    detect_image_mime(&signature[..read]).ok_or_else(|| {
        PluginError::unsupported_media_type(
            "Only JPEG, PNG, GIF, and WebP image uploads are supported",
        )
    })
}

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn safe_original_name(name: Option<&str>) -> String {
    let basename = name
        .unwrap_or("upload")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("upload")
        .trim();
    let cleaned: String = basename
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect();
    if cleaned.is_empty() {
        "upload".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_canonical_and_path_free() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(parse_reference(&format!("media:{id}")), Some(id));
        for invalid in [
            id,
            "media:0123",
            "media:0123456789ABCDEF0123456789ABCDEF",
            "media:../../etc/passwd.............",
            "other:0123456789abcdef0123456789abcdef",
        ] {
            assert_eq!(parse_reference(invalid), None, "accepted `{invalid}`");
        }
    }

    #[test]
    fn detects_only_the_supported_image_signatures() {
        assert_eq!(
            detect_image_mime(b"\x89PNG\r\n\x1a\nrest"),
            Some("image/png")
        );
        assert_eq!(detect_image_mime(b"\xff\xd8\xffrest"), Some("image/jpeg"));
        assert_eq!(detect_image_mime(b"GIF89arest"), Some("image/gif"));
        assert_eq!(detect_image_mime(b"RIFF1234WEBPrest"), Some("image/webp"));
        assert_eq!(detect_image_mime(b"<svg onload=alert(1) />"), None);
        assert_eq!(detect_image_mime(b"not an image"), None);
    }

    #[test]
    fn strips_client_paths_and_control_characters_from_names() {
        assert_eq!(
            safe_original_name(Some(r"C:\fakepath\photo.png")),
            "photo.png"
        );
        assert_eq!(safe_original_name(Some("../../photo\n.png")), "photo.png");
        assert_eq!(safe_original_name(Some("\n\r")), "upload");
        assert_eq!(safe_original_name(None), "upload");
    }
}
