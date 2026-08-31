use sea_orm::entity::prelude::*;

/// An administrator-owned permanent URL redirect.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "redirects_rules")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(unique, indexed)]
    pub source_path: String,
    pub destination_path: String,
    #[sea_orm(indexed, default_value = true)]
    pub enabled: bool,
    pub created_by: String,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
}

vespera::schema_type!(Schema from Model, name = "RedirectsRulesSchema");
impl ActiveModelBehavior for ActiveModel {}
