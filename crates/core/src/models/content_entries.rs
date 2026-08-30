use sea_orm::entity::prelude::*;

/// Typed collection entries with framework-owned publication metadata
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "content_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(indexed)]
    pub collection: String,
    pub title: String,
    pub slug: String,
    #[sea_orm(indexed)]
    pub status: String,
    #[sea_orm(indexed)]
    pub author: String,
    pub fields: Json,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(indexed)]
    pub published_at: Option<DateTimeWithTimeZone>,
}

// Index definitions (SeaORM uses Statement builders externally)
// (unnamed) on [collection]
// (unnamed) on [status]
// (unnamed) on [author]
// (unnamed) on [created_at]
// (unnamed) on [published_at]

/// Composite unique constraints — declare in migrations or use Statement builder.
pub const COMPOSITE_UNIQUES: &[&[&str]] = &[
    &["collection", "slug"], // uq_content_entries_collection_slug
];
impl ActiveModelBehavior for ActiveModel {}
