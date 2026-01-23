//! Auth configuration

use std::time::Duration;

/// Authentication configuration
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret key for signing tokens
    pub jwt_secret: String,

    /// Access token expiry duration (default: 1 hour)
    pub access_token_expiry: Duration,

    /// Refresh token expiry duration (default: 7 days)
    pub refresh_token_expiry: Duration,

    /// Superadmin configuration (optional)
    pub superadmin: Option<SuperadminConfig>,

    /// Public routes that don't require authentication
    pub public_routes: Vec<String>,

    /// Default redirect path when unauthenticated (default: /signin)
    pub signin_redirect: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            access_token_expiry: Duration::from_secs(60 * 60), // 1 hour
            refresh_token_expiry: Duration::from_secs(60 * 60 * 24 * 7), // 7 days
            superadmin: None,
            public_routes: vec![
                "/signin".to_string(),
                "/api/auth/login".to_string(),
                "/api/auth/refresh".to_string(),
                "/health".to_string(),
            ],
            signin_redirect: "/signin".to_string(),
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

    /// Set superadmin credentials
    pub fn superadmin(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.superadmin = Some(SuperadminConfig {
            username: username.into(),
            password: password.into(),
        });
        self
    }

    /// Add a public route
    pub fn public_route(mut self, route: impl Into<String>) -> Self {
        self.public_routes.push(route.into());
        self
    }

    /// Set signin redirect path
    pub fn signin_redirect(mut self, path: impl Into<String>) -> Self {
        self.signin_redirect = path.into();
        self
    }

    /// Check if a path is a public route
    pub fn is_public_route(&self, path: &str) -> bool {
        self.public_routes.iter().any(|r| path.starts_with(r))
    }
}

/// Superadmin configuration
#[derive(Debug, Clone)]
pub struct SuperadminConfig {
    pub username: String,
    pub password: String,
}
