use sea_orm::entity::prelude::*;

/// Validated settings stored once per registered plugin
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub plugin_name: String,
    pub value: Json,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {}
