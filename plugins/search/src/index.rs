//! SQLite FTS5 document storage and content-event synchronization.

use anyhow::{bail, Context};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, Statement,
    TransactionTrait,
};
use serde::Deserialize;
use serde_json::Value;
use yeollin_plugin::yeollin_core::ContentSnapshot;
use yeollin_plugin::{EventEnvelope, InlineSubscriberFuture, SubscriberRegistration};

const SUBJECT_CONTENT: &str = "content";

const CREATE_DOCUMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS search_documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    collection TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    url TEXT NOT NULL,
    status TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (subject, subject_id)
)
"#;

const CREATE_FILTER_INDEX: &str = r#"
CREATE INDEX IF NOT EXISTS search_documents_filters
ON search_documents (subject, collection, status)
"#;

const CREATE_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts USING fts5(
    title,
    body,
    content = 'search_documents',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
)
"#;

const CREATE_INSERT_TRIGGER: &str = r#"
CREATE TRIGGER IF NOT EXISTS search_documents_after_insert
AFTER INSERT ON search_documents BEGIN
    INSERT INTO search_documents_fts(rowid, title, body)
    VALUES (new.id, new.title, new.body);
END
"#;

const CREATE_DELETE_TRIGGER: &str = r#"
CREATE TRIGGER IF NOT EXISTS search_documents_after_delete
AFTER DELETE ON search_documents BEGIN
    INSERT INTO search_documents_fts(search_documents_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
END
"#;

const CREATE_UPDATE_TRIGGER: &str = r#"
CREATE TRIGGER IF NOT EXISTS search_documents_after_update
AFTER UPDATE ON search_documents BEGIN
    INSERT INTO search_documents_fts(search_documents_fts, rowid, title, body)
    VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO search_documents_fts(rowid, title, body)
    VALUES (new.id, new.title, new.body);
END
"#;

const UPSERT_CONTENT: &str = r#"
INSERT INTO search_documents (
    subject,
    subject_id,
    collection,
    title,
    body,
    url,
    status,
    updated_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT (subject, subject_id) DO UPDATE SET
    collection = excluded.collection,
    title = excluded.title,
    body = excluded.body,
    url = excluded.url,
    status = excluded.status,
    updated_at = excluded.updated_at
"#;

const CONTENT_EVENTS: [&str; 5] = [
    yeollin_plugin::yeollin_core::CONTENT_CREATED_EVENT,
    yeollin_plugin::yeollin_core::CONTENT_UPDATED_EVENT,
    yeollin_plugin::yeollin_core::CONTENT_PUBLISHED_EVENT,
    yeollin_plugin::yeollin_core::CONTENT_UNPUBLISHED_EVENT,
    yeollin_plugin::yeollin_core::CONTENT_DELETED_EVENT,
];

#[derive(Deserialize)]
struct ContentEventPayload {
    content: ContentSnapshot,
}

pub(crate) async fn initialize(db: DatabaseConnection) -> anyhow::Result<()> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        bail!("the search plugin requires SQLite with FTS5 support");
    }

    let transaction = db.begin().await?;
    ensure_schema(&transaction).await?;
    synchronize_content(&transaction).await?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) fn content_subscriber() -> SubscriberRegistration {
    SubscriberRegistration::inline("index-content", CONTENT_EVENTS, index_content_event)
}

fn index_content_event<'a>(
    event: EventEnvelope,
    transaction: &'a DatabaseTransaction,
) -> InlineSubscriberFuture<'a> {
    Box::pin(async move {
        let event_name = event.name.clone();
        let payload: ContentEventPayload = serde_json::from_value(event.payload)
            .with_context(|| format!("{event_name} carried an invalid content snapshot"))?;

        if event_name == yeollin_plugin::yeollin_core::CONTENT_DELETED_EVENT {
            delete_content(transaction, &payload.content.id).await?;
        } else if CONTENT_EVENTS.contains(&event_name.as_str()) {
            upsert_content(transaction, &payload.content).await?;
        } else {
            bail!("search received unsupported event `{event_name}`");
        }
        Ok(())
    })
}

async fn ensure_schema<C>(db: &C) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    for statement in [
        CREATE_DOCUMENTS,
        CREATE_FILTER_INDEX,
        CREATE_FTS,
        CREATE_INSERT_TRIGGER,
        CREATE_DELETE_TRIGGER,
        CREATE_UPDATE_TRIGGER,
    ] {
        db.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn synchronize_content<C>(db: &C) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    let snapshots = yeollin_plugin::yeollin_core::list_content_snapshots(db).await?;
    for snapshot in snapshots {
        upsert_content(db, &snapshot).await?;
    }

    db.execute_unprepared(
        "DELETE FROM search_documents WHERE subject = 'content' AND subject_id NOT IN (SELECT id FROM content_entries)",
    )
    .await?;
    db.execute_unprepared(
        "INSERT INTO search_documents_fts(search_documents_fts) VALUES ('rebuild')",
    )
    .await
    .context("SQLite FTS5 is unavailable or the search index could not be rebuilt")?;
    Ok(())
}

async fn upsert_content<C>(db: &C, content: &ContentSnapshot) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    let body = searchable_body(content);
    let url = format!("/content/{}", content.collection);
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        UPSERT_CONTENT,
        [
            SUBJECT_CONTENT.into(),
            content.id.clone().into(),
            content.collection.clone().into(),
            content.title.clone().into(),
            body.into(),
            url.into(),
            content.status.as_str().into(),
            content.updated_at.clone().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn delete_content<C>(db: &C, id: &str) -> anyhow::Result<()>
where
    C: ConnectionTrait,
{
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "DELETE FROM search_documents WHERE subject = ? AND subject_id = ?",
        [SUBJECT_CONTENT.into(), id.to_string().into()],
    ))
    .await?;
    Ok(())
}

fn searchable_body(content: &ContentSnapshot) -> String {
    let mut values = vec![
        content.collection.clone(),
        content.slug.clone(),
        content.author.clone(),
    ];
    collect_json_text(&content.fields, &mut values);
    values.join(" ")
}

fn collect_json_text(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Null => {}
        Value::Bool(value) => output.push(value.to_string()),
        Value::Number(value) => output.push(value.to_string()),
        Value::String(value) => {
            let value = value.trim();
            if !value.is_empty() {
                output.push(value.to_string());
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_json_text(value, output);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_json_text(value, output);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
    use serde::{Deserialize, Serialize};
    use vespera::Schema;
    use yeollin_plugin::{ContentFields, ContentPatch, ContentRepository, EventBus, NewContent};

    use super::*;

    #[derive(Clone, Debug, Default, Serialize, Deserialize, Schema)]
    struct TestFields {
        body: String,
        nested: Vec<String>,
    }

    impl ContentFields for TestFields {}

    async fn matching_ids(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<String>, sea_orm::DbErr> {
        db.query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT d.subject_id FROM search_documents_fts JOIN search_documents d ON d.id = search_documents_fts.rowid WHERE search_documents_fts MATCH ? ORDER BY d.subject_id",
            [query.into()],
        ))
        .await?
        .into_iter()
        .map(|row| row.try_get("", "subject_id"))
        .collect()
    }

    #[tokio::test]
    async fn startup_backfills_and_inline_events_keep_content_current() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        yeollin_plugin::yeollin_core::migrate_core(&db)
            .await
            .unwrap();
        let repository = ContentRepository::<TestFields>::new(db.clone(), "pages");
        let initial_events = EventBus::new(db.clone(), []).unwrap();
        let backfilled = repository
            .create(
                &initial_events,
                "admin",
                NewContent {
                    title: "Existing handbook".to_string(),
                    slug: "existing-handbook".to_string(),
                    fields: TestFields {
                        body: "Backfilled guidance".to_string(),
                        nested: vec!["operating procedure".to_string()],
                    },
                },
            )
            .await
            .unwrap();

        initialize(db.clone()).await.unwrap();
        let matches = matching_ids(&db, "\"Backfilled\"").await.unwrap();
        assert_eq!(matches.as_slice(), std::slice::from_ref(&backfilled.id));

        let events =
            EventBus::new(db.clone(), [content_subscriber().for_plugin("search")]).unwrap();
        let created = repository
            .create(
                &events,
                "editor",
                NewContent {
                    title: "Release checklist".to_string(),
                    slug: "release-checklist".to_string(),
                    fields: TestFields {
                        body: "Verify the deployment".to_string(),
                        nested: vec!["rollback plan".to_string()],
                    },
                },
            )
            .await
            .unwrap();
        let matches = matching_ids(&db, "\"deployment\"").await.unwrap();
        assert_eq!(matches.as_slice(), std::slice::from_ref(&created.id));

        repository
            .update(
                &events,
                "editor",
                &created.id,
                ContentPatch {
                    title: Some("Launch checklist".to_string()),
                    slug: None,
                    fields: Some(TestFields {
                        body: "Confirm observability".to_string(),
                        nested: vec!["incident owner".to_string()],
                    }),
                },
            )
            .await
            .unwrap();
        assert!(matching_ids(&db, "\"deployment\"")
            .await
            .unwrap()
            .is_empty());
        let matches = matching_ids(&db, "\"observability\"").await.unwrap();
        assert_eq!(matches.as_slice(), std::slice::from_ref(&created.id));

        repository
            .delete(&events, "admin", &created.id)
            .await
            .unwrap();
        assert!(matching_ids(&db, "\"observability\"")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn an_index_write_failure_rolls_back_the_content_write() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        yeollin_plugin::yeollin_core::migrate_core(&db)
            .await
            .unwrap();
        initialize(db.clone()).await.unwrap();
        let events =
            EventBus::new(db.clone(), [content_subscriber().for_plugin("search")]).unwrap();
        let repository = ContentRepository::<TestFields>::new(db.clone(), "pages");
        let created = repository
            .create(
                &events,
                "editor",
                NewContent {
                    title: "Atomic title".to_string(),
                    slug: "atomic-title".to_string(),
                    fields: TestFields {
                        body: "Original body".to_string(),
                        nested: vec![],
                    },
                },
            )
            .await
            .unwrap();

        db.execute_unprepared("DROP TABLE search_documents")
            .await
            .unwrap();
        assert!(repository
            .update(
                &events,
                "editor",
                &created.id,
                ContentPatch {
                    title: Some("Must roll back".to_string()),
                    slug: None,
                    fields: None,
                },
            )
            .await
            .is_err());
        assert_eq!(
            repository.get(&created.id).await.unwrap().title,
            "Atomic title"
        );
    }

    #[test]
    fn nested_scalar_fields_become_searchable_without_field_names() {
        let content = ContentSnapshot {
            id: "id".to_string(),
            collection: "pages".to_string(),
            title: "Title".to_string(),
            slug: "a-page".to_string(),
            status: yeollin_plugin::ContentStatus::Draft,
            author: "admin".to_string(),
            fields: serde_json::json!({
                "body": "A searchable sentence",
                "nested": [42, true, null]
            }),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            published_at: None,
        };

        assert_eq!(
            searchable_body(&content),
            "pages a-page admin A searchable sentence 42 true"
        );
    }
}
