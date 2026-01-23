//! JWT Claims

use serde::{Deserialize, Serialize};

/// Token type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
}

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID or username)
    pub sub: String,

    /// Token type (access or refresh)
    pub token_type: TokenType,

    /// Issued at (Unix timestamp)
    pub iat: i64,

    /// Expiration time (Unix timestamp)
    pub exp: i64,

    /// User role (e.g., "superadmin", "admin", "user")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Additional custom data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Claims {
    /// Create new claims
    pub fn new(sub: impl Into<String>, token_type: TokenType, exp: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            sub: sub.into(),
            token_type,
            iat: now,
            exp,
            role: None,
            data: None,
        }
    }

    /// Set role
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Set custom data
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.exp < now
    }

    /// Check if this is an access token
    pub fn is_access_token(&self) -> bool {
        self.token_type == TokenType::Access
    }

    /// Check if this is a refresh token
    pub fn is_refresh_token(&self) -> bool {
        self.token_type == TokenType::Refresh
    }
}
