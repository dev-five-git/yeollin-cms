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
/// Checks JWT token and handles routing based on authentication state:
/// - Public routes: Always accessible
/// - Guest routes: Only accessible when NOT logged in (redirects to dashboard if logged in)
/// - Protected routes: Requires valid token (redirects to signin if not logged in)
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

    // Static assets don't need auth
    if path.starts_with("/_next/") || path.starts_with("/static/") || path.ends_with(".ico") {
        return next.run(request).await;
    }

    // Check if path is public (accessible regardless of auth status)
    if state.config.is_public_route(&path) {
        return next.run(request).await;
    }

    // Try to extract and verify token
    let token = extract_token(&request);
    let valid_user = token.and_then(|t| {
        verify_token(&state.config, &t)
            .ok()
            .filter(|claims| claims.is_access_token())
            .map(CurrentUser::from)
    });

    // Check if path is guest route (only accessible when NOT logged in)
    if state.config.is_guest_route(&path) {
        if valid_user.is_some() {
            // User is logged in, redirect to dashboard
            return guest_redirect(&state.config, &path);
        }
        // User is not logged in, allow access to guest route
        return next.run(request).await;
    }

    // Protected route - require valid token
    match valid_user {
        Some(user) => {
            request.extensions_mut().insert(user);
            next.run(request).await
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

/// Return redirect to dashboard for logged-in users on guest routes
fn guest_redirect(config: &AuthConfig, path: &str) -> Response {
    if path.starts_with("/api/") {
        // For API routes, return 403 Forbidden (authenticated but not allowed)
        (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "Already authenticated",
                "code": "ALREADY_AUTHENTICATED"
            })),
        )
            .into_response()
    } else {
        Redirect::temporary(&config.dashboard_redirect).into_response()
    }
}
