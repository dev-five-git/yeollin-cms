//! Auth error types

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Missing token")]
    MissingToken,

    #[error("Token creation failed: {0}")]
    TokenCreation(String),

    #[error("Password hashing failed: {0}")]
    PasswordHash(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error(
        "JWT secret is {len} bytes; at least {min} are required. \
         Set JWT_SECRET to a random value, e.g. `openssl rand -base64 48`."
    )]
    WeakJwtSecret { len: usize, min: usize },
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: &'static str,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, "INVALID_CREDENTIALS"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "TOKEN_EXPIRED"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "INVALID_TOKEN"),
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "MISSING_TOKEN"),
            AuthError::TokenCreation(_) => (StatusCode::INTERNAL_SERVER_ERROR, "TOKEN_ERROR"),
            AuthError::PasswordHash(_) => (StatusCode::INTERNAL_SERVER_ERROR, "HASH_ERROR"),
            AuthError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            AuthError::WeakJwtSecret { .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, "WEAK_JWT_SECRET")
            }
        };

        let body = ErrorResponse {
            error: self.to_string(),
            code,
        };

        (status, axum::Json(body)).into_response()
    }
}
