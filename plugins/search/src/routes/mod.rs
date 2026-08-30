//! Ranked search API, mounted at `/api/search`.

use axum::{extract::Query, Extension, Json};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, Value};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{Authorize, CurrentUser, PluginError, PluginResult};

const DEFAULT_PAGE_SIZE: u64 = 20;
const MAX_PAGE_SIZE: u64 = 50;
const MAX_PAGE: u64 = 10_000;
const MAX_QUERY_CHARS: usize = 200;
const MAX_QUERY_TERMS: usize = 12;
const MAX_COLLECTION_CHARS: usize = 64;

#[derive(Debug, Clone, Copy, Deserialize, Schema)]
#[serde(rename_all = "lowercase")]
pub enum SearchStatusFilter {
    Draft,
    Published,
}

impl SearchStatusFilter {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
        }
    }
}

#[derive(Debug, Default, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub collection: Option<String>,
    pub status: Option<SearchStatusFilter>,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub subject: String,
    pub id: String,
    pub collection: String,
    pub title: String,
    pub excerpt: String,
    pub url: String,
    pub status: String,
    pub updated_at: String,
    pub relevance: f64,
}

#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// Search indexed content with title-weighted SQLite FTS5 ranking.
#[vespera::route(get, tags = ["search"])]
pub async fn search(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, PluginError> {
    current.require_role("admin")?;
    Ok(Json(search_documents(&db, query).await?))
}

async fn search_documents(
    db: &DatabaseConnection,
    query: SearchQuery,
) -> PluginResult<SearchResponse> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        tracing::error!("search route received a non-SQLite database");
        return Err(PluginError::internal());
    }

    let raw_query = query
        .q
        .as_deref()
        .ok_or_else(|| PluginError::bad_request("q is required"))?;
    let display_query = raw_query.trim().to_string();
    let match_query = build_match_query(raw_query)?;
    let collection = normalize_collection(query.collection.as_deref())?;
    let page = query.page.unwrap_or(1).clamp(1, MAX_PAGE);
    let page_size = query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);

    let mut predicate = String::from("search_documents_fts MATCH ? AND d.subject = 'content'");
    let mut values: Vec<Value> = vec![match_query.into()];
    if let Some(collection) = &collection {
        predicate.push_str(" AND d.collection = ?");
        values.push(collection.clone().into());
    }
    if let Some(status) = query.status {
        predicate.push_str(" AND d.status = ?");
        values.push(status.as_str().into());
    }

    let count_sql = format!(
        "SELECT COUNT(*) AS total \
         FROM search_documents_fts \
         JOIN search_documents d ON d.id = search_documents_fts.rowid \
         WHERE {predicate}"
    );
    let total = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            count_sql,
            values.clone(),
        ))
        .await?
        .ok_or_else(PluginError::internal)?
        .try_get::<i64>("", "total")?;
    let total = u64::try_from(total).unwrap_or_default();

    let offset = (page - 1) * page_size;
    let result_sql = format!(
        "SELECT \
             d.subject AS subject, \
             d.subject_id AS subject_id, \
             d.collection AS collection, \
             d.title AS title, \
             snippet(search_documents_fts, 1, '', '', ' … ', 28) AS excerpt, \
             d.url AS url, \
             d.status AS status, \
             d.updated_at AS updated_at, \
             -bm25(search_documents_fts, 8.0, 1.0) AS relevance \
         FROM search_documents_fts \
         JOIN search_documents d ON d.id = search_documents_fts.rowid \
         WHERE {predicate} \
         ORDER BY bm25(search_documents_fts, 8.0, 1.0), d.updated_at DESC, d.id ASC \
         LIMIT ? OFFSET ?"
    );
    values.push(i64::try_from(page_size).unwrap_or(i64::MAX).into());
    values.push(i64::try_from(offset).unwrap_or(i64::MAX).into());

    let results = db
        .query_all_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            result_sql,
            values,
        ))
        .await?
        .into_iter()
        .map(|row| {
            Ok(SearchResult {
                subject: row.try_get("", "subject")?,
                id: row.try_get("", "subject_id")?,
                collection: row.try_get("", "collection")?,
                title: row.try_get("", "title")?,
                excerpt: row.try_get("", "excerpt")?,
                url: row.try_get("", "url")?,
                status: row.try_get("", "status")?,
                updated_at: row.try_get("", "updated_at")?,
                relevance: row.try_get("", "relevance")?,
            })
        })
        .collect::<Result<Vec<_>, sea_orm::DbErr>>()?;

    Ok(SearchResponse {
        query: display_query,
        results,
        total,
        page,
        page_size,
    })
}

fn build_match_query(value: &str) -> PluginResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(PluginError::bad_request("q must not be empty"));
    }
    if value.chars().count() > MAX_QUERY_CHARS {
        return Err(PluginError::bad_request(format!(
            "q must be at most {MAX_QUERY_CHARS} characters"
        )));
    }

    let terms: Vec<&str> = value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        return Err(PluginError::bad_request(
            "q must contain at least one letter or number",
        ));
    }
    if terms.len() > MAX_QUERY_TERMS {
        return Err(PluginError::bad_request(format!(
            "q must contain at most {MAX_QUERY_TERMS} terms"
        )));
    }

    Ok(terms
        .into_iter()
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" AND "))
}

fn normalize_collection(value: Option<&str>) -> PluginResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() > MAX_COLLECTION_CHARS
        || value.starts_with('-')
        || value.ends_with('-')
        || value.contains("--")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(PluginError::bad_request(
            "collection must be lowercase kebab-case",
        ));
    }
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts_operators_are_plain_terms_and_every_term_is_prefix_matched() {
        assert_eq!(
            build_match_query("alpha OR beta*").unwrap(),
            "\"alpha\"* AND \"OR\"* AND \"beta\"*"
        );
        assert_eq!(
            build_match_query("검색 문서").unwrap(),
            "\"검색\"* AND \"문서\"*"
        );
    }

    #[test]
    fn empty_operator_only_and_oversized_queries_are_refused() {
        assert!(build_match_query("  ---  ").is_err());
        assert!(build_match_query(&"x".repeat(MAX_QUERY_CHARS + 1)).is_err());
        assert!(build_match_query(&vec!["x"; MAX_QUERY_TERMS + 1].join(" ")).is_err());
    }

    #[test]
    fn collection_filters_are_canonical() {
        assert_eq!(
            normalize_collection(Some(" pages ")).unwrap(),
            Some("pages".to_string())
        );
        for invalid in ["Pages/../../secret", "-pages", "pages-", "all--pages"] {
            assert!(normalize_collection(Some(invalid)).is_err());
        }
    }
}
