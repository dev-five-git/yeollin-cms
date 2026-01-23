//! Content types for Yeollin CMS

use serde::{Deserialize, Serialize};
use vespera::Schema;

/// Generic content metadata
#[derive(Debug, Clone, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ContentMeta {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub status: ContentStatus,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub published_at: Option<String>,
}

/// Content status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ContentStatus {
    #[default]
    Draft,
    Review,
    Published,
    Archived,
}
