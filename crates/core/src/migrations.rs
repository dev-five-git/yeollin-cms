//! Framework-owned database migrations.

use sea_orm::DatabaseConnection;

/// Apply all core migrations before plugin initialization.
pub async fn migrate_core(db: &DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(db).await?;
    Ok(())
}

/// Backward-compatible name for callers introduced with typed settings.
pub async fn migrate_settings(db: &DatabaseConnection) -> anyhow::Result<()> {
    migrate_core(db).await
}
