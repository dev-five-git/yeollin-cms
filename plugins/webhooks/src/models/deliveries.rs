use sea_orm::entity::prelude::*;

/// Per-endpoint webhook delivery and dead-letter state
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "webhook_deliveries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    #[sea_orm(indexed)]
    pub webhook_id: String,
    #[sea_orm(indexed)]
    pub event_id: i64,
    #[sea_orm(indexed)]
    pub event_name: String,
    #[sea_orm(indexed)]
    pub status: String,
    #[sea_orm(default_value = 0)]
    pub attempts: i32,
    pub response_status: Option<i32>,
    pub last_error: Option<String>,
    #[sea_orm(indexed, default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
    #[sea_orm(default_value = "NOW()")]
    pub updated_at: DateTimeWithTimeZone,
    pub delivered_at: Option<DateTimeWithTimeZone>,
}

// Index definitions (SeaORM uses Statement builders externally)
// (unnamed) on [webhook_id]
// (unnamed) on [event_id]
// (unnamed) on [event_name]
// (unnamed) on [status]
// (unnamed) on [created_at]

/// Composite unique constraints — declare in migrations or use Statement builder.
pub const COMPOSITE_UNIQUES: &[&[&str]] = &[
    &["webhook_id", "event_id"], // uq_webhook_deliveries_endpoint_event
];
vespera::schema_type!(Schema from Model, name = "DeliveriesSchema");
impl ActiveModelBehavior for ActiveModel {}
