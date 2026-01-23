//! Built-in authentication routes for Yeollin CMS

use axum::{
    http::{header, HeaderMap},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use vespera::Schema;
use yeollin_auth::{generate_token, verify_token, AuthConfig, AuthError};

/// Shared auth config for routes
#[derive(Clone)]
pub struct SharedAuthConfig(pub Arc<AuthConfig>);

/// Login request body
#[derive(Debug, Deserialize, Schema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Serialize, Schema)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// User info response
#[derive(Debug, Serialize, Schema)]
pub struct UserInfoResponse {
    pub username: String,
    pub role: String,
}

/// Refresh token request
#[derive(Debug, Deserialize, Schema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// Create auth router with all auth endpoints
pub fn auth_router(config: Arc<AuthConfig>) -> Router {
    let shared_config = SharedAuthConfig(config);

    Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/auth/refresh", post(refresh_token))
        .layer(Extension(shared_config))
}

/// Login endpoint - authenticates user and returns JWT tokens
#[vespera::route(post, path = "/api/auth/login", tags = ["auth"])]
pub async fn login(
    Extension(config): Extension<SharedAuthConfig>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    // Check superadmin credentials
    if let Some(superadmin) = &config.0.superadmin {
        if body.username == superadmin.username {
            // For superadmin, we compare plain password (in production, should be hashed)
            // But for initial setup, plain comparison is acceptable
            if body.password == superadmin.password {
                let token_pair =
                    generate_token(&config.0, &body.username, Some("superadmin"), None)?;

                return Ok(Json(LoginResponse {
                    access_token: token_pair.access_token,
                    refresh_token: token_pair.refresh_token,
                    token_type: token_pair.token_type.to_string(),
                    expires_in: token_pair.expires_in,
                }));
            }
        }
    }

    // TODO: Add support for database users here
    // For now, only superadmin is supported

    Err(AuthError::InvalidCredentials)
}

/// Get current user info from JWT token
#[vespera::route(get, path = "/api/auth/me", tags = ["auth"])]
pub async fn get_current_user(
    Extension(config): Extension<SharedAuthConfig>,
    headers: HeaderMap,
) -> Result<Json<UserInfoResponse>, AuthError> {
    // Extract token from Authorization header
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingToken)?;

    // Verify token
    let claims = verify_token(&config.0, token)?;

    // Only accept access tokens
    if !claims.is_access_token() {
        return Err(AuthError::InvalidToken);
    }

    Ok(Json(UserInfoResponse {
        username: claims.sub,
        role: claims.role.unwrap_or_else(|| "user".to_string()),
    }))
}

/// Refresh access token using refresh token
#[vespera::route(post, path = "/api/auth/refresh", tags = ["auth"])]
pub async fn refresh_token(
    Extension(config): Extension<SharedAuthConfig>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    // Verify refresh token
    let claims = verify_token(&config.0, &body.refresh_token)?;

    // Only accept refresh tokens
    if !claims.is_refresh_token() {
        return Err(AuthError::InvalidToken);
    }

    // Generate new token pair
    let token_pair = generate_token(&config.0, &claims.sub, claims.role.as_deref(), claims.data)?;

    Ok(Json(LoginResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        token_type: token_pair.token_type.to_string(),
        expires_in: token_pair.expires_in,
    }))
}
