//! Yeollin CMS Authentication
//!
//! JWT token management, password hashing, and auth middleware for Yeollin CMS.

mod claims;
mod config;
mod error;
mod jwt;
mod middleware;
mod password;

pub use claims::{Claims, TokenType};
pub use config::{AuthConfig, SuperadminConfig};
pub use error::AuthError;
pub use jwt::{generate_access_token, generate_token, verify_token, TokenPair};
pub use middleware::{auth_middleware, extract_token, AuthState, CurrentUser};
pub use password::{hash_password, verify_password};
