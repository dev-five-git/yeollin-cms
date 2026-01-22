//! Plugin metadata definitions

use crate::FrontendAssets;
use axum::Router;

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
        }
    }
}
