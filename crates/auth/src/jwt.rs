//! JWT token generation and verification

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::claims::{Claims, TokenType};
use crate::config::AuthConfig;
use crate::error::AuthError;

/// Token pair (access + refresh tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

/// Generate access and refresh tokens for a user
pub fn generate_token(
    config: &AuthConfig,
    subject: &str,
    role: Option<&str>,
    data: Option<serde_json::Value>,
) -> Result<TokenPair, AuthError> {
    let now = chrono::Utc::now().timestamp();

    // Access token
    let access_exp = now + config.access_token_expiry.as_secs() as i64;
    let mut access_claims = Claims::new(subject, TokenType::Access, access_exp);
    if let Some(r) = role {
        access_claims = access_claims.with_role(r);
    }
    if let Some(d) = data.clone() {
        access_claims = access_claims.with_data(d);
    }

    let access_token = encode(
        &Header::default(),
        &access_claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AuthError::TokenCreation(e.to_string()))?;

    // Refresh token
    let refresh_exp = now + config.refresh_token_expiry.as_secs() as i64;
    let mut refresh_claims = Claims::new(subject, TokenType::Refresh, refresh_exp);
    if let Some(r) = role {
        refresh_claims = refresh_claims.with_role(r);
    }
    if let Some(d) = data {
        refresh_claims = refresh_claims.with_data(d);
    }

    let refresh_token = encode(
        &Header::default(),
        &refresh_claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AuthError::TokenCreation(e.to_string()))?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: config.access_token_expiry.as_secs() as i64,
    })
}

/// Verify and decode a JWT token
pub fn verify_token(config: &AuthConfig, token: &str) -> Result<Claims, AuthError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
        _ => AuthError::InvalidToken,
    })?;

    Ok(token_data.claims)
}

/// Generate only an access token (for token refresh)
pub fn generate_access_token(
    config: &AuthConfig,
    subject: &str,
    role: Option<&str>,
    data: Option<serde_json::Value>,
) -> Result<String, AuthError> {
    let now = chrono::Utc::now().timestamp();
    let exp = now + config.access_token_expiry.as_secs() as i64;

    let mut claims = Claims::new(subject, TokenType::Access, exp);
    if let Some(r) = role {
        claims = claims.with_role(r);
    }
    if let Some(d) = data {
        claims = claims.with_data(d);
    }

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AuthError::TokenCreation(e.to_string()))
}
