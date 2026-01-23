//! Auth middleware for Axum

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};

use crate::claims::Claims;
use crate::config::AuthConfig;
use crate::jwt::verify_token;

/// Auth state for middleware
#[derive(Clone)]
pub struct AuthState {
    pub config: Arc<AuthConfig>,
}

impl AuthState {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

/// Current user extracted from JWT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub sub: String,
    pub role: Option<String>,
    pub data: Option<serde_json::Value>,
}

impl From<Claims> for CurrentUser {
    fn from(claims: Claims) -> Self {
        Self {
            sub: claims.sub,
            role: claims.role,
            data: claims.data,
        }
    }
}

/// Auth middleware function
/// Checks JWT token and redirects to signin if invalid
///
/// Usage:
/// ```rust,ignore
/// use axum::{Router, middleware};
/// use yeollin_auth::{AuthState, auth_middleware};
///
/// let auth_state = AuthState::new(auth_config);
/// let app = Router::new()
///     .route("/protected", get(handler))
///     .layer(middleware::from_fn_with_state(auth_state, auth_middleware));
/// ```
pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // Check if path is public
    if state.config.is_public_route(&path) {
        return next.run(request).await;
    }

    // Static assets don't need auth
    if path.starts_with("/_next/") || path.starts_with("/static/") || path.ends_with(".ico") {
        return next.run(request).await;
    }

    // Try to extract token from Authorization header or cookie
    let token = extract_token(&request);

    match token {
        Some(token) => {
            match verify_token(&state.config, &token) {
                Ok(claims) => {
                    // Only accept access tokens for API requests
                    if !claims.is_access_token() {
                        return unauthorized_response(&state.config, &path);
                    }

                    // Add user info to request extensions
                    request.extensions_mut().insert(CurrentUser::from(claims));
                    next.run(request).await
                }
                Err(_) => unauthorized_response(&state.config, &path),
            }
        }
        None => unauthorized_response(&state.config, &path),
    }
}

/// Extract token from request (header or cookie)
pub fn extract_token(request: &Request) -> Option<String> {
    // Try Authorization header first
    if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // Try cookie
    if let Some(cookie_header) = request.headers().get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(token) = cookie.strip_prefix("access_token=") {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

/// Return unauthorized response (redirect for pages, 401 for API)
fn unauthorized_response(config: &AuthConfig, path: &str) -> Response {
    if path.starts_with("/api/") {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "Unauthorized",
                "code": "UNAUTHORIZED"
            })),
        )
            .into_response()
    } else {
        Redirect::temporary(&config.signin_redirect).into_response()
    }
}
