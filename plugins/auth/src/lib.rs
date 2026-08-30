//! Database-backed users and sessions for Yeollin CMS.
//!
//! Replaces the framework's built-in credential check. Passwords are stored as
//! Argon2 PHC hashes and refresh tokens are opaque, hashed, and revocable.

pub mod models;
pub mod routes;
pub mod throttle;
pub mod token;

use std::time::Duration;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use yeollin_plugin::yeollin_auth::hash_password;

use crate::models::{session, user};

const ADMIN_USERNAME_VAR: &str = "YEOLLIN_ADMIN_USERNAME";
const ADMIN_PASSWORD_VAR: &str = "YEOLLIN_ADMIN_PASSWORD";

/// How often expired sessions are swept.
///
/// Rotation writes a new row on every refresh, so without a sweep the table
/// grows for the lifetime of the deployment.
pub const SESSION_PRUNE_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Enforced at every entry point, including the bootstrap administrator.
pub const MIN_PASSWORD_LEN: usize = 12;

/// Usernames are stored and compared in this form, so `Admin ` and `admin`
/// cannot become two accounts that look like one.
pub fn normalize_username(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

yeollin_plugin::yeollin_plugin! {
    name: "auth",
    author: "DevFive",
    description: "Database-backed users and sessions",
    on_init: initialize,
}

async fn initialize(db: DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(&db).await?;
    seed_first_admin(&db).await?;

    // Sweep once at startup so a process that never stays up long enough to tick
    // still clears what accumulated while it was down.
    let removed = prune_expired_sessions(&db).await?;
    if removed > 0 {
        tracing::info!(removed, "Pruned expired sessions");
    }
    spawn_session_pruner(db);

    Ok(())
}

/// Delete sessions whose refresh token can no longer be exchanged.
///
/// Revoked-but-unexpired rows are kept until they expire, leaving a short window
/// in which a replayed token is still recognised and refused rather than simply
/// unknown.
pub async fn prune_expired_sessions(db: &DatabaseConnection) -> anyhow::Result<u64> {
    let outcome = session::Entity::delete_many()
        .filter(session::Column::ExpiresAt.lt(chrono::Utc::now()))
        .exec(db)
        .await?;

    Ok(outcome.rows_affected)
}

fn spawn_session_pruner(db: DatabaseConnection) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SESSION_PRUNE_INTERVAL);
        // The first tick completes immediately; startup already swept.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            match prune_expired_sessions(&db).await {
                Ok(0) => {}
                Ok(removed) => tracing::info!(removed, "Pruned expired sessions"),
                Err(error) => tracing::warn!(%error, "Could not prune expired sessions"),
            }
        }
    });
}

/// Create the first administrator, once, from the environment.
///
/// Runs only while the table is empty, so the environment password cannot
/// silently reset an existing account or resurrect a deleted one. The password
/// is hashed immediately and never stored or compared in plaintext.
async fn seed_first_admin(db: &DatabaseConnection) -> anyhow::Result<()> {
    if user::Entity::find().count(db).await? > 0 {
        return Ok(());
    }

    let (Ok(username), Ok(password)) = (
        std::env::var(ADMIN_USERNAME_VAR),
        std::env::var(ADMIN_PASSWORD_VAR),
    ) else {
        tracing::warn!(
            "No users exist and {ADMIN_USERNAME_VAR}/{ADMIN_PASSWORD_VAR} are unset, \
             so nobody can sign in. Set both and restart to create the first administrator."
        );
        return Ok(());
    };

    let username = normalize_username(&username);
    if username.is_empty() {
        anyhow::bail!("{ADMIN_USERNAME_VAR} must not be empty");
    }
    validate_password(&password)
        .map_err(|reason| anyhow::anyhow!("{ADMIN_PASSWORD_VAR} rejected: {reason}"))?;

    let password_hash =
        hash_password(&password).map_err(|error| anyhow::anyhow!("{error}"))?;

    let now = chrono::Utc::now();
    user::ActiveModel {
        username: Set(username.clone()),
        password_hash: Set(password_hash),
        role: Set("admin".to_string()),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    tracing::info!(%username, "Created the first administrator");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_username_is_trimmed_and_lowercased() {
        assert_eq!(normalize_username("  Admin "), "admin");
    }

    #[test]
    fn a_short_password_is_refused() {
        assert!(validate_password(&"a".repeat(MIN_PASSWORD_LEN - 1)).is_err());
    }

    #[test]
    fn a_password_of_exactly_the_minimum_is_accepted() {
        assert!(validate_password(&"a".repeat(MIN_PASSWORD_LEN)).is_ok());
    }

    #[test]
    fn length_counts_characters_rather_than_bytes() {
        // Each Hangul syllable occupies three bytes, so five of them exceed a
        // twelve-byte threshold while falling short of twelve characters.
        let five_characters = "\u{ac00}".repeat(5);

        assert_eq!(five_characters.chars().count(), 5);
        assert!(five_characters.len() > MIN_PASSWORD_LEN);
        assert!(validate_password(&five_characters).is_err());
    }
}
