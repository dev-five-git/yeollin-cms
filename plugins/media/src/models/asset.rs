use sea_orm::entity::prelude::*;

/// Metadata for files held in the runtime media object store
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "media_assets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(unique)]
    pub reference: String,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub uploaded_by: String,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
}

// Index definitions (SeaORM uses Statement builders externally)
// (unnamed) on [reference]
// (unnamed) on [created_at]
vespera::schema_type!(Schema from Model, name = "AssetsSchema");
impl ActiveModelBehavior for ActiveModel {}
