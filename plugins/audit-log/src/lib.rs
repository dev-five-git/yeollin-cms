//! Administrator audit history backed directly by the core event outbox.

mod routes;

use serde::{de, Deserialize, Deserializer, Serialize};
use vespera::Schema;
use yeollin_plugin::{DatabaseConnection, EventEnvelope, SettingsStore, SubscriberRegistration};

const PLUGIN_NAME: &str = "audit-log";
const DEFAULT_RETENTION_DAYS: u32 = 90;
const MAX_RETENTION_DAYS: u32 = 3_650;

/// Retention policy applied to audit-marked event rows.
#[derive(Clone, Debug, Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogSettings {
    pub retention_days: u32,
}

impl Default for AuditLogSettings {
    fn default() -> Self {
        Self {
            retention_days: DEFAULT_RETENTION_DAYS,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAuditLogSettings {
    retention_days: u32,
}

impl<'de> Deserialize<'de> for AuditLogSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawAuditLogSettings::deserialize(deserializer)?;
        if !(1..=MAX_RETENTION_DAYS).contains(&raw.retention_days) {
            return Err(de::Error::custom(format!(
                "retentionDays must be between 1 and {MAX_RETENTION_DAYS}"
            )));
        }
        Ok(Self {
            retention_days: raw.retention_days,
        })
    }
}

pub(crate) fn retention_cutoff(
    settings: &AuditLogSettings,
) -> chrono::DateTime<chrono::FixedOffset> {
    (chrono::Utc::now() - chrono::Duration::days(i64::from(settings.retention_days))).into()
}

async fn enforce_retention(db: &DatabaseConnection) -> anyhow::Result<()> {
    let settings = SettingsStore::read_persisted::<AuditLogSettings>(db, PLUGIN_NAME).await?;
    let deleted =
        yeollin_plugin::yeollin_core::purge_audited_events_before(db, retention_cutoff(&settings))
            .await?;
    if deleted > 0 {
        tracing::info!(deleted, "Pruned expired audit events");
    }
    Ok(())
}

async fn initialize(db: DatabaseConnection) -> anyhow::Result<()> {
    enforce_retention(&db).await
}

async fn enforce_after_event(_event: EventEnvelope, db: DatabaseConnection) -> anyhow::Result<()> {
    enforce_retention(&db).await
}

yeollin_plugin::yeollin_plugin! {
    name: "audit-log",
    author: "DevFive",
    description: "Administrator audit history over the transactional event outbox",
    on_init: initialize,
    settings: AuditLogSettings,
    subscribers: [SubscriberRegistration::deferred(
        "retention",
        [],
        enforce_after_event,
    )],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_days_are_bounded() {
        for invalid in [0, MAX_RETENTION_DAYS + 1] {
            let value = serde_json::json!({ "retentionDays": invalid });
            assert!(serde_json::from_value::<AuditLogSettings>(value).is_err());
        }

        let minimum: AuditLogSettings =
            serde_json::from_value(serde_json::json!({ "retentionDays": 1 })).unwrap();
        assert_eq!(minimum.retention_days, 1);
    }
}
