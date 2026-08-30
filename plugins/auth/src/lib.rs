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

yeollin_plugin::yeollin_plugin! {
    name: "auth",
    author: "DevFive",
    description: "Database-backed users and sessions",
    on_init: initialize,
    frontend: false,
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

    let username = username.trim().to_lowercase();
    if username.is_empty() || password.is_empty() {
        anyhow::bail!("{ADMIN_USERNAME_VAR} and {ADMIN_PASSWORD_VAR} must not be empty");
    }

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
