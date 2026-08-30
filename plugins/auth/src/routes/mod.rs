//! auth plugin routes, mounted under the plugin API namespace.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{ConnectInfo, Path},
    Extension, Json,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use vespera::Schema;
use yeollin_plugin::{
    yeollin_auth::{generate_access_token, hash_password, verify_password},
    Authorize, AuthConfig, CurrentUser, PluginError,
};

use crate::models::{session, user};
use crate::{normalize_username, validate_password, throttle, token};

/// Roles this plugin will store.
///
/// Role matching is exact, so `Editor` would grant nothing while looking
/// deliberate. An unrecognised role is refused at the boundary rather than
/// written and discovered later.
const KNOWN_ROLES: [&str; 2] = ["admin", "user"];

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

#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub id: i32,
    pub username: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ListUsersResponse {
    pub users: Vec<UserSummary>,
    pub total: u64,
}

#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleRequest {
    pub role: String,
}

#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    pub new_password: String,
}

#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserResponse {
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
        .filter(user::Column::Username.eq(normalize_username(&body.username)))
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

/// List every account. Administrators only.
#[vespera::route(get, path = "/users", tags = ["auth"])]
pub async fn list_users(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
) -> Result<Json<ListUsersResponse>, PluginError> {
    // Authentication alone would let any signed-in account read the roster.
    current.require_role("admin")?;

    let accounts = user::Entity::find().all(&db).await?;

    Ok(Json(ListUsersResponse {
        total: accounts.len() as u64,
        users: accounts.into_iter().map(summarize).collect(),
    }))
}

fn summarize(account: user::Model) -> UserSummary {
    UserSummary {
        id: account.id,
        username: account.username,
        role: account.role,
        created_at: account.created_at.to_rfc3339(),
    }
}

fn validate_role(raw: &str) -> Result<String, PluginError> {
    let role = raw.trim().to_lowercase();
    if KNOWN_ROLES.contains(&role.as_str()) {
        return Ok(role);
    }
    Err(PluginError::bad_request(format!(
        "Unknown role. Allowed roles are {}.",
        KNOWN_ROLES.join(", ")
    )))
}

async fn find_account(db: &DatabaseConnection, id: i32) -> Result<user::Model, PluginError> {
    user::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| PluginError::not_found("No such user"))
}

/// Refuse a change that would leave the deployment with no administrator.
///
/// Nobody could then promote one back, since promotion itself requires the role.
/// Recovery would mean editing the database by hand.
async fn guard_last_administrator(
    db: &DatabaseConnection,
    account: &user::Model,
) -> Result<(), PluginError> {
    if account.role != "admin" {
        return Ok(());
    }

    let administrators = user::Entity::find()
        .filter(user::Column::Role.eq("admin"))
        .count(db)
        .await?;

    if administrators <= 1 {
        return Err(PluginError::conflict(
            "This is the only administrator. Promote another account first.",
        ));
    }
    Ok(())
}

/// End every session of an account whose password changed.
///
/// A password change is how a compromise is contained, so a refresh token minted
/// before it must stop working.
async fn revoke_sessions(db: &DatabaseConnection, user_id: i32) -> Result<(), PluginError> {
    session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Create an account. Administrators only.
#[vespera::route(post, path = "/users", tags = ["auth"])]
pub async fn create_user(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserSummary>, PluginError> {
    current.require_role("admin")?;

    let username = normalize_username(&body.username);
    if username.is_empty() {
        return Err(PluginError::bad_request("Username must not be empty"));
    }
    validate_password(&body.password).map_err(PluginError::bad_request)?;
    let role = validate_role(&body.role)?;

    let password_hash = hash_password(&body.password).map_err(|error| {
        tracing::error!(%error, "could not hash password");
        PluginError::internal()
    })?;

    let now = chrono::Utc::now();
    let created = user::ActiveModel {
        username: Set(username),
        password_hash: Set(password_hash),
        role: Set(role),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(&db)
    .await;

    // The unique index decides, not a prior lookup: two concurrent creates would
    // both pass a check-then-insert and one would surface as a 500.
    match created {
        Ok(account) => Ok(Json(summarize(account))),
        Err(error) => Err(match error.sql_err() {
            Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) => {
                PluginError::conflict("That username is taken")
            }
            _ => PluginError::from(error),
        }),
    }
}

/// Change an account's role. Administrators only.
#[vespera::route(patch, path = "/users/{id}", tags = ["auth"])]
pub async fn update_user_role(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<i32>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<Json<UserSummary>, PluginError> {
    current.require_role("admin")?;

    let role = validate_role(&body.role)?;
    let account = find_account(&db, id).await?;

    if role != "admin" {
        guard_last_administrator(&db, &account).await?;
    }

    let mut changed: user::ActiveModel = account.into();
    changed.role = Set(role);
    changed.updated_at = Set(chrono::Utc::now().into());

    Ok(Json(summarize(changed.update(&db).await?)))
}

/// Delete an account and every session it holds. Administrators only.
#[vespera::route(delete, path = "/users/{id}", tags = ["auth"])]
pub async fn delete_user(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<i32>,
) -> Result<Json<DeleteUserResponse>, PluginError> {
    current.require_role("admin")?;

    let account = find_account(&db, id).await?;

    if account.username == normalize_username(&current.sub) {
        return Err(PluginError::conflict(
            "You cannot delete the account you are signed in as.",
        ));
    }
    guard_last_administrator(&db, &account).await?;

    revoke_sessions(&db, account.id).await?;
    user::Entity::delete_by_id(account.id).exec(&db).await?;

    Ok(Json(DeleteUserResponse { success: true }))
}

/// Change your own password. Every session ends, so sign in again afterwards.
#[vespera::route(post, path = "/password", tags = ["auth"])]
pub async fn change_password(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<DeleteUserResponse>, PluginError> {
    validate_password(&body.new_password).map_err(PluginError::bad_request)?;

    let account = user::Entity::find()
        .filter(user::Column::Username.eq(normalize_username(&current.sub)))
        .one(&db)
        .await?
        .ok_or_else(invalid_credentials)?;

    // Proving the current password is what stops a stolen access token from
    // becoming a permanent takeover.
    let matches = verify_password(&body.current_password, &account.password_hash).map_err(
        |error| {
            tracing::error!(%error, "password verification failed");
            PluginError::internal()
        },
    )?;
    if !matches {
        return Err(invalid_credentials());
    }

    apply_new_password(&db, account, &body.new_password).await
}

/// Reset another account's password. Administrators only.
#[vespera::route(post, path = "/users/{id}/password", tags = ["auth"])]
pub async fn reset_password(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<i32>,
    Json(body): Json<ResetPasswordRequest>,
) -> Result<Json<DeleteUserResponse>, PluginError> {
    current.require_role("admin")?;
    validate_password(&body.new_password).map_err(PluginError::bad_request)?;

    let account = find_account(&db, id).await?;
    apply_new_password(&db, account, &body.new_password).await
}

async fn apply_new_password(
    db: &DatabaseConnection,
    account: user::Model,
    new_password: &str,
) -> Result<Json<DeleteUserResponse>, PluginError> {
    let password_hash = hash_password(new_password).map_err(|error| {
        tracing::error!(%error, "could not hash password");
        PluginError::internal()
    })?;

    let user_id = account.id;
    let mut changed: user::ActiveModel = account.into();
    changed.password_hash = Set(password_hash);
    changed.updated_at = Set(chrono::Utc::now().into());
    changed.update(db).await?;

    revoke_sessions(db, user_id).await?;

    Ok(Json(DeleteUserResponse { success: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn a_known_role_is_accepted() {
        assert_eq!(validate_role("admin").unwrap(), "admin");
        assert_eq!(validate_role("user").unwrap(), "user");
    }

    #[test]
    fn a_role_is_normalised_before_matching() {
        assert_eq!(validate_role("  ADMIN ").unwrap(), "admin");
    }

    #[test]
    fn an_unknown_role_is_refused_rather_than_stored() {
        let refused = validate_role("editor").unwrap_err();

        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn the_refusal_names_the_roles_that_would_work() {
        let refused = validate_role("editor").unwrap_err();
        let body = format!("{refused:?}");

        assert!(body.contains("admin"), "got: {body}");
        assert!(body.contains("user"), "got: {body}");
    }
}
