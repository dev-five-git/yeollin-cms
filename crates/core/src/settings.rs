//! Typed plugin settings registration and persistence.

use std::{any::TypeId, collections::HashMap, sync::Arc};

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::{models::settings, PluginSettingsInfo};

type NormalizeFn = fn(Value) -> Result<Value, SettingsError>;

/// A plugin's compile-time settings contract.
#[derive(Clone, Debug)]
pub struct SettingsRegistration {
    pub plugin_name: &'static str,
    pub schema: Value,
    pub default_value: Value,
    pub api_path: &'static str,
    pub page_path: &'static str,
    pub custom_page: bool,
    type_id: TypeId,
    normalize: NormalizeFn,
}

impl SettingsRegistration {
    /// Register a serializable, deserializable, schema-bearing settings type.
    pub fn new<T>(
        plugin_name: &'static str,
        schema: Value,
        api_path: &'static str,
        page_path: &'static str,
        custom_page: bool,
    ) -> Self
    where
        T: Serialize + DeserializeOwned + Default + vespera::Schema + Send + Sync + 'static,
    {
        let default_value = serde_json::to_value(T::default())
            .expect("plugin settings Default must serialize as JSON");

        Self {
            plugin_name,
            schema,
            default_value,
            api_path,
            page_path,
            custom_page,
            type_id: TypeId::of::<T>(),
            normalize: normalize::<T>,
        }
    }

    pub fn export_info(&self) -> PluginSettingsInfo {
        PluginSettingsInfo {
            schema: self.schema.clone(),
            default_value: self.default_value.clone(),
            api_path: self.api_path.to_string(),
            page_path: self.page_path.to_string(),
            custom_page: self.custom_page,
        }
    }
}

fn normalize<T>(value: Value) -> Result<Value, SettingsError>
where
    T: Serialize + DeserializeOwned,
{
    let typed: T =
        serde_json::from_value(value).map_err(|error| SettingsError::Invalid(error.to_string()))?;
    serde_json::to_value(typed).map_err(SettingsError::Serialize)
}

/// Errors returned by the settings extension point.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("plugin `{0}` has no registered settings type")]
    UnknownPlugin(String),
    #[error("requested settings type does not match plugin `{0}`")]
    TypeMismatch(String),
    #[error("settings are invalid: {0}")]
    Invalid(String),
    #[error("settings could not be serialized: {0}")]
    Serialize(serde_json::Error),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

/// Cloneable Axum extension used by plugin handlers to read typed settings.
#[derive(Clone)]
pub struct SettingsStore {
    db: DatabaseConnection,
    registrations: Arc<HashMap<&'static str, SettingsRegistration>>,
}

impl SettingsStore {
    pub fn new(
        db: DatabaseConnection,
        registrations: impl IntoIterator<Item = SettingsRegistration>,
    ) -> Result<Self, SettingsError> {
        let mut by_name = HashMap::new();
        for registration in registrations {
            if by_name
                .insert(registration.plugin_name, registration.clone())
                .is_some()
            {
                return Err(SettingsError::Invalid(format!(
                    "duplicate settings registration for `{}`",
                    registration.plugin_name
                )));
            }
        }

        Ok(Self {
            db,
            registrations: Arc::new(by_name),
        })
    }

    /// Insert defaults for newly installed plugins without overwriting existing values.
    pub async fn initialize(&self) -> Result<(), SettingsError> {
        let transaction = self.db.begin().await?;
        for registration in self.registrations.values() {
            if settings::Entity::find_by_id(registration.plugin_name)
                .one(&transaction)
                .await?
                .is_none()
            {
                settings::ActiveModel {
                    plugin_name: Set(registration.plugin_name.to_string()),
                    value: Set(registration.default_value.clone()),
                    updated_at: Set(chrono::Utc::now().into()),
                }
                .insert(&transaction)
                .await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Read and deserialize one plugin's settings.
    pub async fn get<T>(&self, plugin_name: &str) -> Result<T, SettingsError>
    where
        T: DeserializeOwned + 'static,
    {
        let registration = self.registration(plugin_name)?;
        if registration.type_id != TypeId::of::<T>() {
            return Err(SettingsError::TypeMismatch(plugin_name.to_string()));
        }

        serde_json::from_value(self.get_json(plugin_name).await?)
            .map_err(|error| SettingsError::Invalid(error.to_string()))
    }

    /// Validate, serialize, and persist one plugin's typed settings.
    pub async fn set<T>(&self, plugin_name: &str, value: T) -> Result<T, SettingsError>
    where
        T: Serialize + DeserializeOwned + 'static,
    {
        let registration = self.registration(plugin_name)?;
        if registration.type_id != TypeId::of::<T>() {
            return Err(SettingsError::TypeMismatch(plugin_name.to_string()));
        }

        let stored = self
            .set_json(
                plugin_name,
                serde_json::to_value(&value).map_err(SettingsError::Serialize)?,
            )
            .await?;
        serde_json::from_value(stored).map_err(|error| SettingsError::Invalid(error.to_string()))
    }

    pub async fn get_json(&self, plugin_name: &str) -> Result<Value, SettingsError> {
        let registration = self.registration(plugin_name)?;
        Ok(settings::Entity::find_by_id(plugin_name)
            .one(&self.db)
            .await?
            .map_or_else(|| registration.default_value.clone(), |row| row.value))
    }

    pub async fn set_json(&self, plugin_name: &str, value: Value) -> Result<Value, SettingsError> {
        let registration = self.registration(plugin_name)?;
        let value = (registration.normalize)(value)?;
        let transaction = self.db.begin().await?;
        let now = chrono::Utc::now();

        if let Some(existing) = settings::Entity::find_by_id(plugin_name)
            .one(&transaction)
            .await?
        {
            let mut active: settings::ActiveModel = existing.into();
            active.value = Set(value.clone());
            active.updated_at = Set(now.into());
            active.update(&transaction).await?;
        } else {
            settings::ActiveModel {
                plugin_name: Set(plugin_name.to_string()),
                value: Set(value.clone()),
                updated_at: Set(now.into()),
            }
            .insert(&transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(value)
    }

    fn registration(&self, plugin_name: &str) -> Result<&SettingsRegistration, SettingsError> {
        self.registrations
            .get(plugin_name)
            .ok_or_else(|| SettingsError::UnknownPlugin(plugin_name.to_string()))
    }
}

/// Apply framework-owned settings migrations before any plugin initializer runs.
pub async fn migrate_settings(db: &DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use vespera::Schema;

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Schema)]
    struct ExampleSettings {
        enabled: bool,
        label: String,
    }

    fn registration() -> SettingsRegistration {
        SettingsRegistration::new::<ExampleSettings>(
            "example",
            serde_json::to_value(vespera::schema!(ExampleSettings)).unwrap(),
            "/api/example/settings",
            "/example/settings",
            false,
        )
    }

    async fn store() -> SettingsStore {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        migrate_settings(&db).await.unwrap();
        let store = SettingsStore::new(db, [registration()]).unwrap();
        store.initialize().await.unwrap();
        store
    }

    #[tokio::test]
    async fn initializes_and_reads_the_typed_default() {
        let store = store().await;

        assert_eq!(
            store.get::<ExampleSettings>("example").await.unwrap(),
            ExampleSettings::default()
        );
    }

    #[tokio::test]
    async fn validates_and_persists_typed_updates() {
        let store = store().await;
        let expected = ExampleSettings {
            enabled: true,
            label: "Ready".to_string(),
        };

        store.set("example", expected.clone()).await.unwrap();

        assert_eq!(
            store.get::<ExampleSettings>("example").await.unwrap(),
            expected
        );
    }

    #[tokio::test]
    async fn rejects_unknown_plugins_and_wrong_types() {
        let store = store().await;

        assert!(matches!(
            store.get::<ExampleSettings>("missing").await,
            Err(SettingsError::UnknownPlugin(_))
        ));
        assert!(matches!(
            store.get::<String>("example").await,
            Err(SettingsError::TypeMismatch(_))
        ));
    }
}
