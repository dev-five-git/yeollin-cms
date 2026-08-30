//! /api/auth routes backed by the users and sessions tables.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::ConnectInfo, Extension, Json};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{
    yeollin_auth::{generate_access_token, verify_password},
    AuthConfig, CurrentUser, PluginError,
};

use crate::models::{session, user};
use crate::{throttle, token};

#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

// Snake_case on the wire, unlike the rest of the API: these two types keep the
// token contract the sign-in page was already written against.
#[derive(Deserialize, Schema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize, Schema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub username: String,
    pub role: String,
}

#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct LogoutResponse {
    pub success: bool,
}

/// Identical for an unknown user and a wrong password, so the endpoint cannot be
/// used to enumerate valid usernames.
fn invalid_credentials() -> PluginError {
    PluginError::unauthorized("Invalid credentials").with_code("INVALID_CREDENTIALS")
}

fn invalid_token() -> PluginError {
    PluginError::unauthorized("Invalid token").with_code("INVALID_TOKEN")
}

async fn issue_tokens(
    db: &DatabaseConnection,
    config: &AuthConfig,
    account: &user::Model,
) -> Result<Json<TokenResponse>, PluginError> {
    let access_token = generate_access_token(config, &account.username, Some(&account.role), None)
        .map_err(|error| {
            tracing::error!(%error, "failed to sign access token");
            PluginError::internal().with_code("TOKEN_ERROR")
        })?;

    let refresh_token = token::generate_refresh_token();
    let expires_at = chrono::Utc::now()
        + chrono::Duration::from_std(config.refresh_token_expiry)
            .unwrap_or_else(|_| chrono::Duration::days(7));

    session::ActiveModel {
        user_id: Set(account.id),
        refresh_token_hash: Set(token::hash_refresh_token(&refresh_token)),
        expires_at: Set(expires_at.into()),
        revoked_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: config.access_token_expiry.as_secs() as i64,
    }))
}

/// Exchange a username and password for a token pair
#[vespera::route(post, path = "/login", tags = ["auth"])]
pub async fn login(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<AuthConfig>>,
    ConnectInfo(source): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, PluginError> {
    let bucket = throttle::key(&body.username, &source.ip().to_string());

    if throttle::is_locked(&bucket) {
        tracing::warn!(source = %source.ip(), "sign-in throttled");
        return Err(PluginError::too_many_requests(
            "Too many attempts. Try again later.",
        ));
    }

    // A missing account still counts as a failure, so absent usernames cannot be
    // probed at a higher rate than real ones.
    let Some(account) = user::Entity::find()
        .filter(user::Column::Username.eq(body.username.trim().to_lowercase()))
        .one(&db)
        .await?
    else {
        throttle::record_failure(&bucket);
        return Err(invalid_credentials());
    };

    let matches = verify_password(&body.password, &account.password_hash).map_err(|error| {
        tracing::error!(%error, "password verification failed");
        PluginError::internal()
    })?;

    if !matches {
        throttle::record_failure(&bucket);
        return Err(invalid_credentials());
    }

    throttle::clear(&bucket);
    issue_tokens(&db, &config, &account).await
}

/// Exchange a refresh token for a new pair, invalidating the presented one
#[vespera::route(post, path = "/refresh", tags = ["auth"])]
pub async fn refresh(
    Extension(db): Extension<DatabaseConnection>,
    Extension(config): Extension<Arc<AuthConfig>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, PluginError> {
    let presented = token::hash_refresh_token(&body.refresh_token);

    let stored = session::Entity::find()
        .filter(session::Column::RefreshTokenHash.eq(presented))
        .one(&db)
        .await?
        .ok_or_else(invalid_token)?;

    let now = chrono::Utc::now();
    if stored.revoked_at.is_some() || stored.expires_at < now {
        return Err(invalid_token());
    }

    // Revoked before the replacement is minted, so replaying the presented token
    // can never yield a second valid pair.
    let user_id = stored.user_id;
    let mut spent: session::ActiveModel = stored.into();
    spent.revoked_at = Set(Some(now.into()));
    spent.update(&db).await?;

    let account = user::Entity::find_by_id(user_id)
        .one(&db)
        .await?
        .ok_or_else(invalid_token)?;

    issue_tokens(&db, &config, &account).await
}

/// Revoke the presented refresh token
#[vespera::route(post, path = "/logout", tags = ["auth"])]
pub async fn logout(
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<LogoutResponse>, PluginError> {
    let presented = token::hash_refresh_token(&body.refresh_token);

    if let Some(stored) = session::Entity::find()
        .filter(session::Column::RefreshTokenHash.eq(presented))
        .one(&db)
        .await?
    {
        if stored.revoked_at.is_none() {
            let mut spent: session::ActiveModel = stored.into();
            spent.revoked_at = Set(Some(chrono::Utc::now().into()));
            spent.update(&db).await?;
        }
    }

    // Always reports success: whether the token existed is not the caller's business.
    Ok(Json(LogoutResponse { success: true }))
}

/// Describe the caller identified by the access token
#[vespera::route(get, path = "/me", tags = ["auth"])]
pub async fn me(Extension(current): Extension<CurrentUser>) -> Json<UserResponse> {
    Json(UserResponse {
        username: current.sub,
        role: current.role.unwrap_or_else(|| "user".to_string()),
    })
}
