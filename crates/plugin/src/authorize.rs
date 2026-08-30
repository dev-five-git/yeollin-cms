//! Role checks for plugin route handlers.
//!
//! Authentication only establishes *who* is calling. Until a handler asks, every
//! signed-in caller reaches every protected route, so a plugin that exposes
//! destructive or administrative endpoints must state the role it requires.
//!
//! ```rust,ignore
//! pub async fn delete_everything(
//!     Extension(current): Extension<CurrentUser>,
//! ) -> Result<Json<Done>, PluginError> {
//!     current.require_role("admin")?;
//!     // ...
//! }
//! ```

use yeollin_auth::CurrentUser;

use crate::error::{PluginError, PluginResult};

/// Role checks on the caller established by the auth middleware.
pub trait Authorize {
    /// Whether the caller holds exactly this role.
    ///
    /// Comparison is exact: roles are identifiers, not prose, and a
    /// case-insensitive match would let `Admin` and `admin` drift apart.
    fn has_role(&self, role: &str) -> bool;

    /// Reject the caller unless they hold this role.
    fn require_role(&self, role: &str) -> PluginResult<()>;

    /// Reject the caller unless they hold at least one of these roles.
    fn require_any_role(&self, roles: &[&str]) -> PluginResult<()>;
}

impl Authorize for CurrentUser {
    fn has_role(&self, role: &str) -> bool {
        self.role.as_deref() == Some(role)
    }

    fn require_role(&self, role: &str) -> PluginResult<()> {
        if self.has_role(role) {
            return Ok(());
        }

        Err(denied())
    }

    fn require_any_role(&self, roles: &[&str]) -> PluginResult<()> {
        if roles.iter().any(|role| self.has_role(role)) {
            return Ok(());
        }

        Err(denied())
    }
}

/// The response is identical whichever role was missing, so a caller cannot map
/// out the role model by probing endpoints.
fn denied() -> PluginError {
    PluginError::forbidden("Insufficient permissions")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caller(role: Option<&str>) -> CurrentUser {
        CurrentUser {
            sub: "someone".to_string(),
            role: role.map(str::to_string),
            data: None,
        }
    }

    #[test]
    fn the_matching_role_passes() {
        assert!(caller(Some("admin")).require_role("admin").is_ok());
        assert!(caller(Some("admin")).has_role("admin"));
    }

    #[test]
    fn a_different_role_is_refused() {
        let refused = caller(Some("editor")).require_role("admin").unwrap_err();

        assert_eq!(refused.status(), axum::http::StatusCode::FORBIDDEN);
    }

    #[test]
    fn a_caller_without_a_role_is_refused() {
        assert!(caller(None).require_role("admin").is_err());
        assert!(!caller(None).has_role("admin"));
    }

    #[test]
    fn role_matching_is_exact() {
        // `Admin` and `admin` must not be treated as the same role.
        assert!(caller(Some("Admin")).require_role("admin").is_err());
        assert!(caller(Some("admin ")).require_role("admin").is_err());
    }

    #[test]
    fn any_of_several_roles_passes() {
        let editor = caller(Some("editor"));

        assert!(editor.require_any_role(&["admin", "editor"]).is_ok());
        assert!(editor.require_any_role(&["admin", "owner"]).is_err());
        assert!(editor.require_any_role(&[]).is_err());
    }

    #[test]
    fn refusals_do_not_reveal_the_required_role() {
        let refused = caller(Some("editor")).require_role("admin").unwrap_err();

        assert_eq!(refused.code(), "FORBIDDEN");
    }
}
