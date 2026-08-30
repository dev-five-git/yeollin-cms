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

    // Frontend assets don't need auth
    if is_frontend_asset(&path, state.config.dev_mode) {
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

/// Bundled output namespaces that exist in both dev and release builds.
const BUILT_ASSET_PREFIXES: &[&str] = &["/_next/", "/static/"];

/// Paths served only by the Vite dev server. They are absent from a release
/// binary, so exempting them outside dev mode would grant an auth bypass for
/// routes the application itself may still answer.
const DEV_ASSET_PREFIXES: &[&str] = &["/@", "/__vite_hmr", "/node_modules/", "/src/", "/df/"];

/// Individually exempt files, matched exactly.
///
/// Matching a *suffix* such as `.ico` here would exempt every path ending in it,
/// letting `/memo/1.ico` reach a handler unauthenticated.
const EXEMPT_FILES: &[&str] = &["/favicon.ico"];

fn is_frontend_asset(path: &str, dev_mode: bool) -> bool {
    if path.contains("..") {
        return false;
    }

    if EXEMPT_FILES.contains(&path) {
        return true;
    }

    if BUILT_ASSET_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return true;
    }

    dev_mode
        && DEV_ASSET_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
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

#[cfg(test)]
mod tests {
    use super::is_frontend_asset;
    use crate::config::AuthConfig;

    const DEV: bool = true;
    const PROD: bool = false;

    #[test]
    fn allows_built_assets_in_every_mode() {
        for path in ["/_next/static/chunk.js", "/static/logo.svg", "/favicon.ico"] {
            assert!(is_frontend_asset(path, PROD), "expected asset: {path}");
            assert!(is_frontend_asset(path, DEV), "expected asset: {path}");
        }
    }

    #[test]
    fn allows_vite_dev_assets_only_in_dev_mode() {
        for path in [
            "/@id/virtual:vite-rsc/entry-browser",
            "/@vite/client",
            "/__vite_hmr",
            "/node_modules/vinext/dist/client.js",
            "/src/app/page.tsx",
            "/df/index.ts",
        ] {
            assert!(is_frontend_asset(path, DEV), "expected dev asset: {path}");
            assert!(
                !is_frontend_asset(path, PROD),
                "dev asset must not bypass auth in production: {path}"
            );
        }
    }

    #[test]
    fn keeps_application_routes_protected() {
        for path in ["/", "/signin", "/example-plugin", "/api/menus"] {
            assert!(
                !is_frontend_asset(path, DEV),
                "expected application path: {path}"
            );
        }
    }

    #[test]
    fn icon_suffix_does_not_exempt_application_routes() {
        for path in [
            "/memo/1.ico",
            "/api/auth/me.ico",
            "/api/menus.ico",
            "/example-plugin/items/secret.ico",
        ] {
            assert!(
                !is_frontend_asset(path, DEV),
                "`.ico` suffix must not bypass auth: {path}"
            );
        }
    }

    #[test]
    fn traversal_attempts_are_never_assets() {
        for path in ["/_next/../memo/1", "/static/../../api/menus", "/src/../memo"] {
            assert!(
                !is_frontend_asset(path, DEV),
                "traversal must not be treated as an asset: {path}"
            );
        }
    }

    #[test]
    fn public_and_guest_routes_match_whole_paths_only() {
        let config = AuthConfig::default();

        assert!(config.is_public_route("/health"));
        assert!(config.is_public_route("/health/"));
        assert!(config.is_public_route("/api/auth/login"));
        assert!(config.is_guest_route("/signin"));

        for path in [
            "/healthz",
            "/health-check",
            "/health/details",
            "/api/auth/login-extra",
            "/api/auth/login/escalate",
        ] {
            assert!(
                !config.is_public_route(path),
                "prefix must not widen public access: {path}"
            );
        }

        for path in ["/signinx", "/signin/reset"] {
            assert!(
                !config.is_guest_route(path),
                "prefix must not widen guest access: {path}"
            );
        }
    }

    #[test]
    fn unknown_routes_stay_protected() {
        let config = AuthConfig::default();

        for path in ["/", "/memo", "/whatever", "/api/unknown"] {
            assert!(!config.is_public_route(path), "unexpectedly public: {path}");
        }
    }

    #[test]
    fn weak_jwt_secrets_are_rejected() {
        assert!(AuthConfig::default().validate().is_err());
        assert!(AuthConfig::new("short").validate().is_err());
        assert!(AuthConfig::new("y".repeat(31)).validate().is_err());
        assert!(AuthConfig::new("y".repeat(32)).validate().is_ok());
    }
}
