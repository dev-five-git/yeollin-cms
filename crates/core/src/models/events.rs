use sea_orm::entity::prelude::*;

/// Transactional event outbox for inline and deferred plugin subscribers
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(indexed)]
    pub name: String,
    pub payload: Json,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(indexed)]
    pub processed_at: Option<DateTimeWithTimeZone>,
    #[sea_orm(default_value = 0)]
    pub delivery_attempts: i32,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub available_at: DateTimeWithTimeZone,
    pub last_error: Option<String>,
}

// Index definitions (SeaORM uses Statement builders externally)
// (unnamed) on [name]
// (unnamed) on [created_at]
// (unnamed) on [processed_at]
// (unnamed) on [available_at]
impl ActiveModelBehavior for ActiveModel {}
