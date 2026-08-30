use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Refresh-token sessions for auth-users
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_sessions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(indexed)]
    pub user_id: i32,
    #[sea_orm(unique)]
    pub refresh_token_hash: String,
    pub expires_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    #[sea_orm(default_value = "NOW()")]
    pub created_at: DateTimeWithTimeZone,
}

impl ActiveModelBehavior for ActiveModel {}
