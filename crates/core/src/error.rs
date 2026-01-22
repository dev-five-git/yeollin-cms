//! Error types for Yeollin CMS

use thiserror::Error;

/// Core error type for Yeollin CMS
#[derive(Error, Debug)]
pub enum YeollinError {
    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for Yeollin CMS
pub type YeollinResult<T> = Result<T, YeollinError>;
