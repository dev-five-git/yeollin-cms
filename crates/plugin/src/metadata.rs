//! Plugin metadata definitions

use crate::FrontendAssets;
use axum::Router;
use sea_orm::DatabaseConnection;
use std::future::Future;
use std::pin::Pin;
use yeollin_core::{SettingsRegistration, SubscriberRegistration};

/// Type alias for plugin initialization function
/// Called when plugin is loaded with database connection available
pub type PluginInitFn = Box<
    dyn Fn(DatabaseConnection) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>
        + Send
        + Sync,
>;

/// Plugin metadata containing all information needed to register a plugin
pub struct PluginMetadata {
    /// Plugin name (used as identifier)
    pub name: &'static str,
    /// Plugin version (semver)
    pub version: &'static str,
    /// Plugin author
    pub author: Option<&'static str>,
    /// Optional description
    pub description: Option<&'static str>,
    /// Plugin license (e.g., "MIT", "Apache-2.0")
    pub license: Option<&'static str>,
    /// Axum router with plugin routes
    pub router: Router,
    /// Frontend assets (TSX, menu config, etc.)
    pub frontend: FrontendAssets,
    /// Frontend path on filesystem (for prebuild)
    pub frontend_path: Option<&'static str>,
    /// Optional initialization callback (called with database connection)
    pub on_init: Option<PluginInitFn>,
    /// Optional typed settings contract.
    pub settings: Option<SettingsRegistration>,
    /// Observe-only event subscribers owned by this plugin.
    pub subscribers: Vec<SubscriberRegistration>,
}

impl PluginMetadata {
    /// Create a new plugin metadata builder
    pub fn builder(name: &'static str, version: &'static str) -> PluginMetadataBuilder {
        PluginMetadataBuilder {
            name,
            version,
            author: None,
            description: None,
            license: None,
            router: Router::new(),
            frontend: FrontendAssets::empty(),
            frontend_path: None,
            on_init: None,
            settings: None,
            subscribers: vec![],
        }
    }
}

/// Builder for PluginMetadata
pub struct PluginMetadataBuilder {
    name: &'static str,
    version: &'static str,
    author: Option<&'static str>,
    description: Option<&'static str>,
    license: Option<&'static str>,
    router: Router,
    frontend: FrontendAssets,
    frontend_path: Option<&'static str>,
    on_init: Option<PluginInitFn>,
    settings: Option<SettingsRegistration>,
    subscribers: Vec<SubscriberRegistration>,
}

impl PluginMetadataBuilder {
    /// Set plugin author
    pub fn author(mut self, author: &'static str) -> Self {
        self.author = Some(author);
        self
    }

    /// Set plugin description
    pub fn description(mut self, desc: &'static str) -> Self {
        self.description = Some(desc);
        self
    }

    /// Set plugin license
    pub fn license(mut self, license: &'static str) -> Self {
        self.license = Some(license);
        self
    }

    /// Set plugin router
    pub fn router(mut self, router: Router) -> Self {
        self.router = router;
        self
    }

    /// Set frontend assets
    pub fn frontend(mut self, frontend: FrontendAssets) -> Self {
        self.frontend = frontend;
        self
    }

    /// Set frontend path for prebuild (usually concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
    pub fn frontend_path(mut self, path: &'static str) -> Self {
        self.frontend_path = Some(path);
        self
    }

    /// Set initialization callback (called with database connection when available)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// PluginMetadata::builder("my-plugin", "1.0.0")
    ///     .on_init(|db| Box::pin(async move {
    ///         // Run migrations, seed data, etc.
    ///         Ok(())
    ///     }))
    ///     .build()
    /// ```
    pub fn on_init<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(DatabaseConnection) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        self.on_init = Some(Box::new(move |db| Box::pin(f(db))));
        self
    }

    /// Register the plugin's typed settings contract.
    pub fn settings(mut self, settings: SettingsRegistration) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Add an Inline or Deferred event subscriber.
    pub fn subscriber(mut self, subscriber: SubscriberRegistration) -> Self {
        self.subscribers.push(subscriber);
        self
    }

    /// Build the plugin metadata
    pub fn build(self) -> PluginMetadata {
        PluginMetadata {
            name: self.name,
            version: self.version,
            author: self.author,
            description: self.description,
            license: self.license,
            router: self.router,
            frontend: self.frontend,
            frontend_path: self.frontend_path,
            on_init: self.on_init,
            settings: self.settings,
            subscribers: self.subscribers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_retains_classified_subscribers() {
        let metadata = PluginMetadata::builder("example", "1.0.0")
            .subscriber(SubscriberRegistration::deferred(
                "observer",
                ["memo.created"],
                |_event, _db| async { Ok(()) },
            ))
            .build();

        assert_eq!(metadata.subscribers.len(), 1);
        assert_eq!(
            metadata.subscribers[0].mode(),
            yeollin_core::SubscriberMode::Deferred
        );
    }
}
