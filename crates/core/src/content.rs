//! Compile-time typed content collections and their shared persistence layer.

use std::{marker::PhantomData, str::FromStr};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, ModelTrait,
    Order, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use vespera::Schema;

use crate::{models::content_entries, ContentCollectionInfo, Event, EventBus};

pub const DEFAULT_CONTENT_PAGE_SIZE: u64 = 20;
pub const MAX_CONTENT_PAGE_SIZE: u64 = 100;
pub const CONTENT_CREATED_EVENT: &str = "content.created";
pub const CONTENT_UPDATED_EVENT: &str = "content.updated";
pub const CONTENT_PUBLISHED_EVENT: &str = "content.published";
pub const CONTENT_UNPUBLISHED_EVENT: &str = "content.unpublished";
pub const CONTENT_DELETED_EVENT: &str = "content.deleted";

/// Fields supplied by a collection author.
///
/// Implementations are concrete Rust types, so request decoding, validation,
/// and OpenAPI remain compile-time checked even though the shared table stores
/// the fields as JSON.
pub trait ContentFields: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

/// The only two publication states exposed by the reusable workflow.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "lowercase")]
pub enum ContentStatus {
    #[default]
    Draft,
    Published,
}

impl ContentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
        }
    }
}

impl FromStr for ContentStatus {
    type Err = ContentError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            other => Err(ContentError::InvalidStoredStatus(other.to_string())),
        }
    }
}

/// One typed entry returned by a collection repository.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRecord<T> {
    pub id: String,
    pub collection: String,
    pub title: String,
    pub slug: String,
    pub status: ContentStatus,
    pub author: String,
    pub fields: T,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

/// Paginated typed collection result.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPage<T> {
    pub entries: Vec<ContentRecord<T>>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// Input shared by generated create handlers.
pub struct NewContent<T> {
    pub title: String,
    pub slug: String,
    pub fields: T,
}

/// Input shared by generated update handlers.
pub struct ContentPatch<T> {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub fields: Option<T>,
}

/// Compile-time collection metadata retained for routing and prebuild.
#[derive(Clone, Debug)]
pub struct ContentCollectionRegistration {
    name: &'static str,
    label: &'static str,
    order: i32,
    schema: Value,
    default_value: Value,
    plugin_name: Option<&'static str>,
    api_path: Option<String>,
    page_path: Option<String>,
    public_api_path: Option<String>,
}

impl ContentCollectionRegistration {
    pub fn new<T>(name: &'static str, label: &'static str, order: i32, schema: Value) -> Self
    where
        T: Serialize + Default,
    {
        assert_valid_collection_name(name);
        assert!(
            !label.trim().is_empty(),
            "content collection label must not be empty"
        );

        Self {
            name,
            label,
            order,
            schema,
            default_value: serde_json::to_value(T::default())
                .expect("content collection Default must serialize as JSON"),
            plugin_name: None,
            api_path: None,
            page_path: None,
            public_api_path: None,
        }
    }

    /// Bind the collection to the plugin namespace that declared it.
    #[must_use]
    pub fn for_plugin(mut self, plugin_name: &'static str, api_prefix: &'static str) -> Self {
        let api_path = format!("{api_prefix}/{}", self.name);
        self.plugin_name = Some(plugin_name);
        self.page_path = Some(format!("/{plugin_name}/{}", self.name));
        self.public_api_path = Some(format!("{api_path}/published"));
        self.api_path = Some(api_path);
        self
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn order(&self) -> i32 {
        self.order
    }

    pub fn plugin_name(&self) -> &'static str {
        self.plugin_name
            .expect("content collection must be assigned to a plugin")
    }

    pub fn api_path(&self) -> &str {
        self.api_path
            .as_deref()
            .expect("content collection must be assigned to a plugin")
    }

    pub fn page_path(&self) -> &str {
        self.page_path
            .as_deref()
            .expect("content collection must be assigned to a plugin")
    }

    pub fn public_api_path(&self) -> &str {
        self.public_api_path
            .as_deref()
            .expect("content collection must be assigned to a plugin")
    }

    pub fn export_info(&self) -> ContentCollectionInfo {
        ContentCollectionInfo {
            name: self.name.to_string(),
            label: self.label.to_string(),
            order: self.order,
            schema: self.schema.clone(),
            default_value: self.default_value.clone(),
            api_path: self.api_path().to_string(),
            page_path: self.page_path().to_string(),
            public_api_path: self.public_api_path().to_string(),
        }
    }
}

fn assert_valid_collection_name(name: &str) {
    let valid = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    assert!(
        valid,
        "content collection names must be 1-64 lowercase kebab-case characters"
    );
}

/// Shared CRUD implementation used by concrete generated collection handlers.
#[derive(Clone)]
pub struct ContentRepository<T> {
    db: DatabaseConnection,
    collection: &'static str,
    fields: PhantomData<fn() -> T>,
}

impl<T> ContentRepository<T>
where
    T: ContentFields,
{
    pub fn new(db: DatabaseConnection, collection: &'static str) -> Self {
        assert_valid_collection_name(collection);
        debug_assert_eq!(
            content_entries::COMPOSITE_UNIQUES,
            &[&["collection", "slug"] as &[&str]],
            "the content repository relies on collection-scoped unique slugs"
        );
        Self {
            db,
            collection,
            fields: PhantomData,
        }
    }

    pub async fn list(
        &self,
        page: u64,
        page_size: u64,
        status: Option<ContentStatus>,
    ) -> Result<ContentPage<T>, ContentError> {
        let page = page.max(1);
        let page_size = page_size.clamp(1, MAX_CONTENT_PAGE_SIZE);
        let mut query = content_entries::Entity::find()
            .filter(content_entries::Column::Collection.eq(self.collection));
        if let Some(status) = status {
            query = query.filter(content_entries::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by(content_entries::Column::UpdatedAt, Order::Desc)
            .paginate(&self.db, page_size);
        let total = paginator.num_items().await?;
        let entries = paginator
            .fetch_page(page - 1)
            .await?
            .into_iter()
            .map(decode_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ContentPage {
            entries,
            total,
            page,
            page_size,
        })
    }

    pub async fn get(&self, id: &str) -> Result<ContentRecord<T>, ContentError> {
        let model = content_entries::Entity::find_by_id(id)
            .filter(content_entries::Column::Collection.eq(self.collection))
            .one(&self.db)
            .await?
            .ok_or(ContentError::NotFound)?;
        decode_record(model)
    }

    pub async fn published(&self, slug: &str) -> Result<ContentRecord<T>, ContentError> {
        let slug = normalize_slug(slug)?;
        let model = content_entries::Entity::find()
            .filter(content_entries::Column::Collection.eq(self.collection))
            .filter(content_entries::Column::Slug.eq(slug))
            .filter(content_entries::Column::Status.eq(ContentStatus::Published.as_str()))
            .one(&self.db)
            .await?
            .ok_or(ContentError::NotFound)?;
        decode_record(model)
    }

    pub async fn create(
        &self,
        events: &EventBus,
        actor: &str,
        input: NewContent<T>,
    ) -> Result<ContentRecord<T>, ContentError> {
        let title = normalize_title(&input.title)?;
        let slug = normalize_slug(&input.slug)?;
        input.fields.validate().map_err(ContentError::Invalid)?;
        let fields = serde_json::to_value(&input.fields)?;
        let now = chrono::Utc::now();
        let mut transaction = events.begin().await?;

        ensure_slug_available(transaction.connection(), self.collection, &slug, None).await?;
        let stored = content_entries::ActiveModel {
            id: Set(random_id()),
            collection: Set(self.collection.to_string()),
            title: Set(title),
            slug: Set(slug),
            status: Set(ContentStatus::Draft.as_str().to_string()),
            author: Set(actor.to_string()),
            fields: Set(fields),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            published_at: Set(None),
        }
        .insert(transaction.connection())
        .await?;
        let record = decode_record(stored)?;
        transaction
            .emit(&ContentCreated {
                actor: actor.to_string(),
                content: ContentSnapshot::from_record(&record)?,
            })
            .await?;
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn update(
        &self,
        events: &EventBus,
        actor: &str,
        id: &str,
        patch: ContentPatch<T>,
    ) -> Result<ContentRecord<T>, ContentError> {
        if patch.title.is_none() && patch.slug.is_none() && patch.fields.is_none() {
            return Err(ContentError::Invalid(
                "an update must include title, slug, or fields".to_string(),
            ));
        }

        let mut transaction = events.begin().await?;
        let model = content_entries::Entity::find_by_id(id)
            .filter(content_entries::Column::Collection.eq(self.collection))
            .one(transaction.connection())
            .await?
            .ok_or(ContentError::NotFound)?;
        let mut active = model.into_active_model();

        if let Some(title) = patch.title {
            active.title = Set(normalize_title(&title)?);
        }
        if let Some(slug) = patch.slug {
            let slug = normalize_slug(&slug)?;
            ensure_slug_available(transaction.connection(), self.collection, &slug, Some(id))
                .await?;
            active.slug = Set(slug);
        }
        if let Some(fields) = patch.fields {
            fields.validate().map_err(ContentError::Invalid)?;
            active.fields = Set(serde_json::to_value(fields)?);
        }
        active.updated_at = Set(chrono::Utc::now().into());
        let stored = active.update(transaction.connection()).await?;
        let record = decode_record(stored)?;
        transaction
            .emit(&ContentUpdated {
                actor: actor.to_string(),
                content: ContentSnapshot::from_record(&record)?,
            })
            .await?;
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn publish(
        &self,
        events: &EventBus,
        actor: &str,
        id: &str,
    ) -> Result<ContentRecord<T>, ContentError> {
        self.transition(events, actor, id, ContentStatus::Published)
            .await
    }

    pub async fn unpublish(
        &self,
        events: &EventBus,
        actor: &str,
        id: &str,
    ) -> Result<ContentRecord<T>, ContentError> {
        self.transition(events, actor, id, ContentStatus::Draft)
            .await
    }

    async fn transition(
        &self,
        events: &EventBus,
        actor: &str,
        id: &str,
        status: ContentStatus,
    ) -> Result<ContentRecord<T>, ContentError> {
        let mut transaction = events.begin().await?;
        let model = content_entries::Entity::find_by_id(id)
            .filter(content_entries::Column::Collection.eq(self.collection))
            .one(transaction.connection())
            .await?
            .ok_or(ContentError::NotFound)?;
        if ContentStatus::from_str(&model.status)? == status {
            transaction.rollback().await?;
            return decode_record(model);
        }

        let now = chrono::Utc::now();
        let mut active = model.into_active_model();
        active.status = Set(status.as_str().to_string());
        active.updated_at = Set(now.into());
        active.published_at = Set(match status {
            ContentStatus::Draft => None,
            ContentStatus::Published => Some(now.into()),
        });
        let stored = active.update(transaction.connection()).await?;
        let record = decode_record(stored)?;
        let snapshot = ContentSnapshot::from_record(&record)?;
        match status {
            ContentStatus::Draft => {
                transaction
                    .emit(&ContentUnpublished {
                        actor: actor.to_string(),
                        content: snapshot,
                    })
                    .await?;
            }
            ContentStatus::Published => {
                transaction
                    .emit(&ContentPublished {
                        actor: actor.to_string(),
                        content: snapshot,
                    })
                    .await?;
            }
        }
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn delete(
        &self,
        events: &EventBus,
        actor: &str,
        id: &str,
    ) -> Result<String, ContentError> {
        let mut transaction = events.begin().await?;
        let model = content_entries::Entity::find_by_id(id)
            .filter(content_entries::Column::Collection.eq(self.collection))
            .one(transaction.connection())
            .await?
            .ok_or(ContentError::NotFound)?;
        let record: ContentRecord<T> = decode_record(model.clone())?;
        model.delete(transaction.connection()).await?;
        transaction
            .emit(&ContentDeleted {
                actor: actor.to_string(),
                content: ContentSnapshot::from_record(&record)?,
            })
            .await?;
        transaction.commit().await?;
        Ok(id.to_string())
    }
}

async fn ensure_slug_available(
    db: &sea_orm::DatabaseTransaction,
    collection: &str,
    slug: &str,
    except_id: Option<&str>,
) -> Result<(), ContentError> {
    let mut query = content_entries::Entity::find()
        .filter(content_entries::Column::Collection.eq(collection))
        .filter(content_entries::Column::Slug.eq(slug));
    if let Some(id) = except_id {
        query = query.filter(content_entries::Column::Id.ne(id));
    }
    if query.one(db).await?.is_some() {
        return Err(ContentError::DuplicateSlug(slug.to_string()));
    }
    Ok(())
}

fn decode_record<T>(model: content_entries::Model) -> Result<ContentRecord<T>, ContentError>
where
    T: DeserializeOwned,
{
    Ok(ContentRecord {
        id: model.id,
        collection: model.collection,
        title: model.title,
        slug: model.slug,
        status: ContentStatus::from_str(&model.status)?,
        author: model.author,
        fields: serde_json::from_value(model.fields)?,
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
        published_at: model.published_at.map(|value| value.to_rfc3339()),
    })
}

fn normalize_title(value: &str) -> Result<String, ContentError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ContentError::Invalid("title must not be empty".to_string()));
    }
    if value.chars().count() > 200 {
        return Err(ContentError::Invalid(
            "title must be at most 200 characters".to_string(),
        ));
    }
    Ok(value.to_string())
}

pub fn normalize_slug(value: &str) -> Result<String, ContentError> {
    let mut slug = String::new();
    let mut separator = false;
    for byte in value.trim().bytes() {
        let byte = byte.to_ascii_lowercase();
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.push(char::from(byte));
        } else if matches!(byte, b'-' | b'_' | b' ' | b'\t') {
            separator = true;
        } else {
            return Err(ContentError::Invalid(
                "slug may contain only ASCII letters, numbers, spaces, underscores, and hyphens"
                    .to_string(),
            ));
        }
    }
    if slug.is_empty() {
        return Err(ContentError::Invalid("slug must not be empty".to_string()));
    }
    if slug.len() > 100 {
        return Err(ContentError::Invalid(
            "slug must be at most 100 characters".to_string(),
        ));
    }
    Ok(slug)
}

fn random_id() -> String {
    rand::random::<[u8; 16]>()
        .iter()
        .fold(String::with_capacity(32), |mut value, byte| {
            use std::fmt::Write;
            write!(value, "{byte:02x}").expect("writing to a String cannot fail");
            value
        })
}

/// Stable JSON shape consumed by audit, webhooks, and the phase-8 search index.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSnapshot {
    pub id: String,
    pub collection: String,
    pub title: String,
    pub slug: String,
    pub status: ContentStatus,
    pub author: String,
    pub fields: Value,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

impl ContentSnapshot {
    fn from_record<T>(record: &ContentRecord<T>) -> Result<Self, serde_json::Error>
    where
        T: Serialize,
    {
        Ok(Self {
            id: record.id.clone(),
            collection: record.collection.clone(),
            title: record.title.clone(),
            slug: record.slug.clone(),
            status: record.status,
            author: record.author.clone(),
            fields: serde_json::to_value(&record.fields)?,
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            published_at: record.published_at.clone(),
        })
    }
}

macro_rules! content_event {
    ($type_name:ident, $event_name:expr) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $type_name {
            pub actor: String,
            pub content: ContentSnapshot,
        }

        impl Event for $type_name {
            const NAME: &'static str = $event_name;
            const AUDIT: bool = true;
        }
    };
}

content_event!(ContentCreated, CONTENT_CREATED_EVENT);
content_event!(ContentUpdated, CONTENT_UPDATED_EVENT);
content_event!(ContentPublished, CONTENT_PUBLISHED_EVENT);
content_event!(ContentUnpublished, CONTENT_UNPUBLISHED_EVENT);
content_event!(ContentDeleted, CONTENT_DELETED_EVENT);

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("content is invalid: {0}")]
    Invalid(String),
    #[error("content entry was not found")]
    NotFound,
    #[error("slug `{0}` is already used by this collection")]
    DuplicateSlug(String),
    #[error("persisted content has unknown status `{0}`")]
    InvalidStoredStatus(String),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Event(#[from] crate::EventError),
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, EntityTrait, PaginatorTrait};

    use super::*;
    use crate::{migrate_core, models::events};

    #[derive(Clone, Debug, Default, Serialize, Deserialize, Schema)]
    struct TestFields {
        body: String,
    }

    impl ContentFields for TestFields {
        fn validate(&self) -> Result<(), String> {
            if self.body.trim().is_empty() {
                return Err("body must not be empty".to_string());
            }
            Ok(())
        }
    }

    async fn repository() -> (ContentRepository<TestFields>, EventBus) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        migrate_core(&db).await.unwrap();
        let events = EventBus::new(db.clone(), std::iter::empty()).unwrap();
        (ContentRepository::new(db, "articles"), events)
    }

    #[test]
    fn collection_registration_is_bound_to_the_plugin_namespace() {
        let registration = ContentCollectionRegistration::new::<TestFields>(
            "articles",
            "Articles",
            30,
            serde_json::to_value(vespera::schema!(TestFields)).unwrap(),
        )
        .for_plugin("content", "/api/content");

        assert_eq!(registration.api_path(), "/api/content/articles");
        assert_eq!(registration.page_path(), "/content/articles");
        assert_eq!(
            registration.public_api_path(),
            "/api/content/articles/published"
        );
        assert_eq!(registration.export_info().default_value["body"], "");
    }

    #[test]
    #[should_panic(expected = "lowercase kebab-case")]
    fn invalid_collection_names_fail_registration() {
        ContentCollectionRegistration::new::<TestFields>(
            "Bad/Articles",
            "Articles",
            30,
            serde_json::json!({}),
        );
    }

    #[test]
    fn slugs_are_canonical_and_path_free() {
        assert_eq!(normalize_slug("  Hello_world  ").unwrap(), "hello-world");
        assert!(normalize_slug("../../secret").is_err());
        assert!(normalize_slug("한글").is_err());
    }

    #[tokio::test]
    async fn draft_publish_update_unpublish_and_delete_share_one_generic() {
        let (repository, events) = repository().await;
        let created = repository
            .create(
                &events,
                "admin",
                NewContent {
                    title: "First article".to_string(),
                    slug: "First Article".to_string(),
                    fields: TestFields {
                        body: "Draft body".to_string(),
                    },
                },
            )
            .await
            .unwrap();
        assert_eq!(created.status, ContentStatus::Draft);
        assert_eq!(created.slug, "first-article");
        assert!(repository.published(&created.slug).await.is_err());

        let published = repository
            .publish(&events, "publisher", &created.id)
            .await
            .unwrap();
        assert_eq!(published.status, ContentStatus::Published);
        assert!(published.published_at.is_some());
        assert_eq!(
            repository.published("first-article").await.unwrap().id,
            created.id
        );

        let updated = repository
            .update(
                &events,
                "editor",
                &created.id,
                ContentPatch {
                    title: Some("Renamed".to_string()),
                    slug: Some("renamed".to_string()),
                    fields: Some(TestFields {
                        body: "Published body".to_string(),
                    }),
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.fields.body, "Published body");
        assert_eq!(repository.list(1, 1, None).await.unwrap().total, 1);

        let draft = repository
            .unpublish(&events, "editor", &created.id)
            .await
            .unwrap();
        assert_eq!(draft.status, ContentStatus::Draft);
        assert!(repository.published("renamed").await.is_err());

        repository
            .delete(&events, "admin", &created.id)
            .await
            .unwrap();
        assert!(repository.get(&created.id).await.is_err());
        assert_eq!(
            events::Entity::find().count(&repository.db).await.unwrap(),
            5
        );
    }

    #[tokio::test]
    async fn invalid_fields_and_duplicate_slugs_are_refused() {
        let (repository, events) = repository().await;
        let invalid = repository
            .create(
                &events,
                "admin",
                NewContent {
                    title: "Invalid".to_string(),
                    slug: "invalid".to_string(),
                    fields: TestFields::default(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(invalid, ContentError::Invalid(_)));

        repository
            .create(
                &events,
                "admin",
                NewContent {
                    title: "One".to_string(),
                    slug: "same".to_string(),
                    fields: TestFields {
                        body: "body".to_string(),
                    },
                },
            )
            .await
            .unwrap();
        let duplicate = repository
            .create(
                &events,
                "admin",
                NewContent {
                    title: "Two".to_string(),
                    slug: "same".to_string(),
                    fields: TestFields {
                        body: "body".to_string(),
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(duplicate, ContentError::DuplicateSlug(_)));
    }
}
