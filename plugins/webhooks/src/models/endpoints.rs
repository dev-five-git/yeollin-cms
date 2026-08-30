use sea_orm::entity::prelude::*;

/// Administrator-configured signed webhook endpoints
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "webhook_endpoints")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(unique)]
    pub name: String,
    pub url: String,
    pub secret: String,
    pub event_names: Json,
    #[sea_orm(default_value = false)]
    pub allow_private_networks: bool,
    #[sea_orm(default_value = 5)]
    pub timeout_seconds: i32,
    #[sea_orm(indexed, default_value = true)]
    pub enabled: bool,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
}

// Index definitions (SeaORM uses Statement builders externally)
// (unnamed) on [name]
// (unnamed) on [enabled]
// (unnamed) on [created_at]
vespera::schema_type!(Schema from Model, name = "EndpointsSchema");
impl ActiveModelBehavior for ActiveModel {}
