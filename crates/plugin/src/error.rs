//! The error type plugin routes return.
//!
//! Defined once here rather than per plugin. Two plugins that each declared
//! their own `ErrorResponse` used to produce two OpenAPI components with the
//! same name, which vespera now rejects at build time — and before it rejected
//! them, one silently overwrote the other.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use vespera::Schema;

/// Body of a failed plugin request.
#[derive(Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub error: String,
    pub code: String,
}

/// A failure from a plugin route handler.
#[derive(Debug)]
pub struct PluginError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl PluginError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "BAD_REQUEST", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", message)
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_ATTEMPTS", message)
    }

    /// A failure the caller can do nothing about.
    ///
    /// Takes no message on purpose: internal failures are logged server-side and
    /// reported generically, so that database and filesystem detail cannot reach
    /// a client.
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Internal server error",
        )
    }

    /// Override the machine-readable code while keeping the status.
    ///
    /// Lets a plugin distinguish causes that share a status, such as an
    /// unauthorized response caused by bad credentials versus a spent token.
    #[must_use]
    pub fn with_code(mut self, code: &'static str) -> Self {
        self.code = code;
        self
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn code(&self) -> &str {
        self.code
    }
}

impl IntoResponse for PluginError {
    fn into_response(self) -> Response {
        (
            self.status,
            axum::Json(ErrorBody {
                error: self.message,
                code: self.code.to_string(),
            }),
        )
            .into_response()
    }
}

/// Database failures are logged with their cause and reported generically.
///
/// The previous per-plugin handlers put `DbErr::to_string()` straight into the
/// response body, which leaked schema and query detail to callers.
impl From<sea_orm::DbErr> for PluginError {
    fn from(error: sea_orm::DbErr) -> Self {
        tracing::error!(%error, "plugin database error");
        Self::internal()
    }
}

/// Shorthand for helper functions that fail with a [`PluginError`].
///
/// **Do not use this in a `#[vespera::route]` handler signature.** Vespera reads
/// the return type syntactically and cannot see through a type alias, so a
/// handler declared as `PluginResult<Json<T>>` loses its `$ref` to `T` and is
/// documented as a bare `{"type":"object"}` — silently, with the build still
/// green. Handlers must spell out `Result<Json<T>, PluginError>`.
pub type PluginResult<T> = Result<T, PluginError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_carry_their_status_and_code() {
        assert_eq!(PluginError::not_found("x").status(), StatusCode::NOT_FOUND);
        assert_eq!(PluginError::not_found("x").code(), "NOT_FOUND");
        assert_eq!(PluginError::conflict("x").status(), StatusCode::CONFLICT);
        assert_eq!(
            PluginError::too_many_requests("x").code(),
            "TOO_MANY_ATTEMPTS"
        );
    }

    #[test]
    fn database_errors_do_not_reach_the_client() {
        let leaky = sea_orm::DbErr::Custom("table auth_users column password_hash".to_string());

        let converted = PluginError::from(leaky);

        assert_eq!(converted.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !converted.message.contains("password_hash"),
            "database detail must not survive into the response body"
        );
    }
}
