//! Yeollin application builder

use std::sync::Arc;
use axum::{Extension, Json, Router};
use include_dir::Dir;
use serde::Serialize;
use vespera::Schema;
use yeollin_core::MenuConfig;
use yeollin_plugin::PluginMetadata;
use crate::state::AppState;
use crate::server::Server;
use crate::static_files::static_router;
use crate::dev_proxy::dev_proxy_router;

/// Shared menus for Extension layer
#[derive(Clone)]
pub struct SharedMenus(pub Arc<Vec<MenuConfig>>);

/// Health check response
#[derive(Serialize, Schema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Plugin info for export
#[derive(Serialize, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub frontend_path: Option<String>,
}

/// Yeollin CMS Application
pub struct YeollinApp {
    router: Router,
    menus: Vec<MenuConfig>,
    plugins: Vec<PluginInfo>,
    state: AppState,
}

impl YeollinApp {
    /// Run the CMS server
    /// 
    /// If YEOLLIN_EXPORT_PLUGINS env var is set, exports plugin info as JSON and exits
    pub async fn run(self) -> anyhow::Result<()> {
        // Check for export mode - output ONLY JSON, nothing else
        if std::env::var("YEOLLIN_EXPORT_PLUGINS").is_ok() {
            // Use eprintln for any debug info, println only for JSON
            println!("{}", self.export_plugins_json());
            return Ok(());
        }

        let server = Server::new(self.router, self.state);
        server.run().await
    }

    /// Get all registered plugins
    pub fn plugins(&self) -> &[PluginInfo] {
        &self.plugins
    }

    /// Get all registered menus
    pub fn menus(&self) -> &[MenuConfig] {
        &self.menus
    }

    /// Export plugins as JSON for CLI prebuild
    pub fn export_plugins_json(&self) -> String {
        serde_json::to_string_pretty(&self.plugins).unwrap_or_default()
    }

    /// Export menus as JSON for frontend consumption
    pub fn export_menus_json(&self) -> String {
        serde_json::to_string_pretty(&self.menus).unwrap_or_default()
    }
}

/// Builder for Yeollin CMS Application
pub struct YeollinAppBuilder {
    plugins: Vec<PluginMetadata>,
    host: String,
    port: u16,
    static_dir: Option<&'static Dir<'static>>,
    dev_proxy_port: Option<u16>,
}

impl YeollinAppBuilder {
    pub(crate) fn new() -> Self {
        Self {
            plugins: vec![],
            host: "0.0.0.0".to_string(),
            port: 3001,
            static_dir: None,
            dev_proxy_port: None,
        }
    }

    /// Register a plugin
    pub fn register_plugin(mut self, metadata: PluginMetadata) -> Self {
        tracing::info!(
            plugin = metadata.name,
            version = metadata.version,
            "Registering plugin"
        );
        self.plugins.push(metadata);
        self
    }

    /// Set the server host
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the server port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the static files directory (embedded Next.js output)
    /// 
    /// When set, the server will serve static files from this directory
    /// as a fallback for routes not matched by API endpoints.
    /// 
    /// # Example
    /// 
    /// ```rust,ignore
    /// use include_dir::{include_dir, Dir};
    /// 
    /// static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../.yeollin/app/out");
    /// 
    /// let app = yeollin::app()
    ///     .with_static(&STATIC_DIR)
    ///     .build();
    /// ```
    pub fn with_static(mut self, dir: &'static Dir<'static>) -> Self {
        self.static_dir = Some(dir);
        self
    }

    /// Set the dev proxy port (for development mode)
    /// 
    /// When set, the server will proxy non-API requests to the Next.js dev server
    /// running on the specified port. This is mutually exclusive with `with_static`.
    /// 
    /// # Example
    /// 
    /// ```rust,ignore
    /// let app = yeollin::app()
    ///     .with_dev_proxy(3000)  // Proxy to Next.js dev server on port 3000
    ///     .build();
    /// ```
    pub fn with_dev_proxy(mut self, port: u16) -> Self {
        self.dev_proxy_port = Some(port);
        self
    }

    /// Build the application
    pub fn build(self) -> YeollinApp {
        let mut router = Router::new();
        let mut menus = vec![];
        let mut plugins = vec![];

        // Merge all plugin routers
        for plugin in self.plugins {
            tracing::info!(
                plugin = plugin.name,
                has_frontend = plugin.frontend.has_frontend(),
                "Merging plugin router"
            );

            // Collect plugin info for export
            plugins.push(PluginInfo {
                name: plugin.name.to_string(),
                version: plugin.version.to_string(),
                frontend_path: plugin.frontend_path.map(|s| s.to_string()),
            });

            // Merge the router
            router = router.merge(plugin.router);

            // Collect menus
            if let Some(menu) = plugin.frontend.menu() {
                menus.push(menu.clone());
            }

            // Log frontend assets
            for tsx_file in plugin.frontend.tsx_files() {
                tracing::debug!(file = tsx_file, "Found frontend asset");
            }
        }

        let state = AppState::new(self.host, self.port, menus.clone());
        let shared_menus = SharedMenus(Arc::new(menus.clone()));

        // Add core routes with vespera
        router = router
            .route("/health", axum::routing::get(health_check))
            .route("/api/menus", axum::routing::get(get_menus))
            .layer(Extension(shared_menus));

        // Add static file serving or dev proxy as fallback
        if let Some(dev_proxy_port) = self.dev_proxy_port {
            router = router.merge(dev_proxy_router(dev_proxy_port));
            tracing::info!("Dev proxy enabled -> http://127.0.0.1:{}", dev_proxy_port);
        } else if let Some(static_dir) = self.static_dir {
            router = router.merge(static_router(static_dir));
            tracing::info!("Static file serving enabled");
        }

        YeollinApp {
            router,
            menus,
            plugins,
            state,
        }
    }
}

/// Health check endpoint
#[vespera::route(get, path = "/health", tags = ["system"])]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Get all registered menus
#[vespera::route(get, path = "/api/menus", tags = ["system"])]
pub async fn get_menus(
    Extension(menus): Extension<SharedMenus>,
) -> Json<Vec<MenuConfig>> {
    Json((*menus.0).clone())
}
