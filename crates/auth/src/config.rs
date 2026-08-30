//! Auth configuration

use std::time::Duration;

use crate::error::AuthError;

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret key for signing tokens
    pub jwt_secret: String,

    /// Access token expiry duration (default: 1 hour)
    pub access_token_expiry: Duration,

    /// Refresh token expiry duration (default: 7 days)
    pub refresh_token_expiry: Duration,

    /// Public routes that don't require authentication
    pub public_routes: Vec<String>,

    /// Guest routes - accessible only when NOT logged in (redirect to dashboard if logged in)
    pub guest_routes: Vec<String>,

    /// Default redirect path when unauthenticated (default: /signin)
    pub signin_redirect: String,

    /// Default redirect path for logged-in users on guest routes (default: /)
    pub dashboard_redirect: String,

    /// Whether dev-server asset paths (Vite virtual modules, HMR socket, raw
    /// sources) may skip authentication. Those paths do not exist in a release
    /// binary, so leaving this on in production only widens the bypass surface.
    pub dev_mode: bool,
}

/// Shortest accepted `jwt_secret`. 32 bytes matches the HS256 output size, below
/// which the signing key weakens the signature rather than the other way round.
pub const MIN_JWT_SECRET_LEN: usize = 32;

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            access_token_expiry: Duration::from_secs(60 * 60), // 1 hour
            refresh_token_expiry: Duration::from_secs(60 * 60 * 24 * 7), // 7 days
            public_routes: vec![
                "/api/auth/login".to_string(),
                "/api/auth/refresh".to_string(),
                // Authenticates with the refresh token it carries, so it must
                // stay reachable once the access token has expired.
                "/api/auth/logout".to_string(),
                "/health".to_string(),
            ],
            guest_routes: vec!["/signin".to_string()],
            signin_redirect: "/signin".to_string(),
            dashboard_redirect: "/".to_string(),
            dev_mode: false,
        }
    }
}

impl AuthConfig {
    /// Create a new AuthConfig with the given JWT secret
    pub fn new(jwt_secret: impl Into<String>) -> Self {
        Self {
            jwt_secret: jwt_secret.into(),
            ..Default::default()
        }
    }

    /// Set access token expiry
    pub fn access_token_expiry(mut self, duration: Duration) -> Self {
        self.access_token_expiry = duration;
        self
    }

    /// Set refresh token expiry
    pub fn refresh_token_expiry(mut self, duration: Duration) -> Self {
        self.refresh_token_expiry = duration;
        self
    }

    /// Add a public route
    pub fn public_route(mut self, route: impl Into<String>) -> Self {
        self.public_routes.push(route.into());
        self
    }

    /// Add a guest route (only accessible when NOT logged in)
    pub fn guest_route(mut self, route: impl Into<String>) -> Self {
        self.guest_routes.push(route.into());
        self
    }

    /// Set signin redirect path
    pub fn signin_redirect(mut self, path: impl Into<String>) -> Self {
        self.signin_redirect = path.into();
        self
    }

    /// Set dashboard redirect path (for logged-in users on guest routes)
    pub fn dashboard_redirect(mut self, path: impl Into<String>) -> Self {
        self.dashboard_redirect = path.into();
        self
    }

    /// Allow dev-server asset paths to skip authentication
    pub fn dev_mode(mut self, enabled: bool) -> Self {
        self.dev_mode = enabled;
        self
    }

    /// Reject configurations that cannot sign trustworthy tokens.
    ///
    /// Callers must run this before serving traffic; an empty or short secret
    /// otherwise produces forgeable sessions rather than a startup failure.
    pub fn validate(&self) -> Result<(), AuthError> {
        if self.jwt_secret.len() < MIN_JWT_SECRET_LEN {
            return Err(AuthError::WeakJwtSecret {
                len: self.jwt_secret.len(),
                min: MIN_JWT_SECRET_LEN,
            });
        }
        Ok(())
    }

    /// Check if a path is a public route
    pub fn is_public_route(&self, path: &str) -> bool {
        matches_any_route(&self.public_routes, path)
    }

    /// Check if a path is a guest route (only for non-authenticated users)
    pub fn is_guest_route(&self, path: &str) -> bool {
        matches_any_route(&self.guest_routes, path)
    }
}

fn matches_any_route(routes: &[String], path: &str) -> bool {
    let path = normalize_path(path);
    routes.iter().any(|route| normalize_path(route) == path)
}

/// Collapse a request path to its comparison form: trailing slashes carry no
/// routing meaning, but any `..` segment is preserved so a traversal attempt can
/// never normalize into a route that was declared public.
fn normalize_path(path: &str) -> &str {
    match path.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    }
}

