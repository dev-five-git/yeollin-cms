//! Yeollin application builder

use crate::auth_routes::auth_router;
use crate::dev_proxy::dev_proxy_router;
use crate::server::Server;
use crate::state::AppState;
use crate::static_files::static_router;
use axum::{middleware, Extension, Json, Router};
use include_dir::Dir;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::sync::Arc;
use vespera::Schema;
use yeollin_auth::{auth_middleware, AuthConfig, AuthState};
use yeollin_core::MenuConfig;
use yeollin_plugin::PluginMetadata;

/// Shared menus for Extension layer
#[derive(Clone)]
pub struct SharedMenus(pub Arc<Vec<MenuConfig>>);

/// Health check response
#[derive(Serialize, Schema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Plugin info for export and API
#[derive(Serialize, Clone, Schema)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend_path: Option<String>,
}

/// Shared plugins for Extension layer
#[derive(Clone)]
pub struct SharedPlugins(pub Arc<Vec<PluginInfo>>);

/// Stored plugin init callback with name for logging
pub struct PluginInitCallback {
    pub name: String,
    pub callback: yeollin_plugin::PluginInitFn,
}

/// Yeollin CMS Application
pub struct YeollinApp {
    router: Router,
    menus: Vec<MenuConfig>,
    plugins: Vec<PluginInfo>,
    state: AppState,
    /// Database connection (if configured)
    database: Option<DatabaseConnection>,
    /// Plugin initialization callbacks
    init_callbacks: Vec<PluginInitCallback>,
}

impl YeollinApp {
    /// Run the CMS server
    ///
    /// If YEOLLIN_EXPORT_MENUS env var is set, exports menus as JSON and exits
    /// If YEOLLIN_EXPORT_PLUGINS env var is set, exports plugin info as JSON and exits
    pub async fn run(self) -> anyhow::Result<()> {
        // Check for menus export mode
        if std::env::var("YEOLLIN_EXPORT_MENUS").is_ok() {
            println!("{}", self.export_menus_json());
            return Ok(());
        }

        // Check for plugins export mode
        if std::env::var("YEOLLIN_EXPORT_PLUGINS").is_ok() {
            println!("{}", self.export_plugins_json());
            return Ok(());
        }

        // Run plugin initialization callbacks if database is available
        if let Some(db) = &self.database {
            for init in &self.init_callbacks {
                tracing::info!(plugin = %init.name, "Running plugin initialization");
                if let Err(e) = (init.callback)(db.clone()).await {
                    tracing::error!(plugin = %init.name, error = %e, "Plugin initialization failed");
                    return Err(anyhow::anyhow!(
                        "Plugin '{}' initialization failed: {}",
                        init.name,
                        e
                    ));
                }
                tracing::info!(plugin = %init.name, "Plugin initialization completed");
            }
        } else if !self.init_callbacks.is_empty() {
            tracing::warn!(
                "Plugins with on_init callbacks registered but no database configured. \
                 Skipping initialization for: {}",
                self.init_callbacks
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
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
    routers: Vec<Router>,
    host: String,
    port: u16,
    static_dir: Option<&'static Dir<'static>>,
    dev_proxy_port: Option<u16>,
    auth_config: Option<AuthConfig>,
    database: Option<DatabaseConnection>,
}

impl YeollinAppBuilder {
    pub(crate) fn new() -> Self {
        Self {
            plugins: vec![],
            routers: vec![],
            host: "0.0.0.0".to_string(),
            port: 3001,
            static_dir: None,
            dev_proxy_port: None,
            auth_config: None,
            database: None,
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

    /// Merge an external router (e.g., vespera-generated routes)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let vespera_router = vespera::vespera!(
    ///     openapi = "openapi.json",
    ///     title = "My API",
    ///     version = "1.0.0",
    ///     docs_url = "/docs"
    /// );
    ///
    /// let app = yeollin::app()
    ///     .merge(vespera_router)
    ///     .build();
    /// ```
    pub fn merge(mut self, router: Router) -> Self {
        self.routers.push(router);
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

    /// Set the static files directory (embedded vinext output)
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
    /// When set, the server will proxy non-API requests to the vinext dev server
    /// running on the specified port. This is mutually exclusive with `with_static`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let app = yeollin::app()
    ///     .with_dev_proxy(3000)  // Proxy to vinext dev server on port 3000
    ///     .build();
    /// ```
    pub fn with_dev_proxy(mut self, port: u16) -> Self {
        self.dev_proxy_port = Some(port);
        self
    }

    /// Configure authentication
    ///
    /// Sets up JWT-based authentication with the provided configuration.
    /// Public routes are automatically detected from the `(public)` directory
    /// during prebuild (stored in `.yeollin/app/src/public-routes.json`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use yeollin_auth::AuthConfig;
    /// use std::time::Duration;
    ///
    /// let app = yeollin::app()
    ///     .with_auth(
    ///         AuthConfig::new("your-secret-key")
    ///             .superadmin("admin", "password")
    ///             .access_token_expiry(Duration::from_secs(3600))
    ///     )
    ///     .build();
    /// ```
    pub fn with_auth(mut self, config: AuthConfig) -> Self {
        self.auth_config = Some(config);
        self
    }

    /// Configure database connection
    ///
    /// Sets up sea-orm database connection that will be available to all routes
    /// via Axum's Extension layer. Plugins can access it by extracting
    /// `Extension<DatabaseConnection>` in their handlers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sea_orm::Database;
    ///
    /// let db = Database::connect("postgres://...").await?;
    ///
    /// let app = yeollin::app()
    ///     .with_database(db)
    ///     .build();
    /// ```
    pub fn with_database(mut self, db: DatabaseConnection) -> Self {
        self.database = Some(db);
        self
    }

    /// Build the application
    pub fn build(mut self) -> YeollinApp {
        // Auto-detect dev proxy from environment
        if self.dev_proxy_port.is_none() {
            if let Ok(dev_proxy_port) = std::env::var("YEOLLIN_DEV_PROXY") {
                if let Ok(port) = dev_proxy_port.parse::<u16>() {
                    self.dev_proxy_port = Some(port);
                }
            }
        }

        let mut router = Router::new();
        let mut menus = vec![];
        let mut plugins = vec![];
        let mut init_callbacks = vec![];

        // Merge external routers (e.g., vespera)
        for external_router in self.routers {
            router = router.merge(external_router);
        }

        // Merge all plugin routers
        for plugin in self.plugins {
            tracing::info!(
                plugin = plugin.name,
                has_frontend = plugin.frontend.has_frontend(),
                has_on_init = plugin.on_init.is_some(),
                "Merging plugin router"
            );

            // Collect plugin info for export
            plugins.push(PluginInfo {
                name: plugin.name.to_string(),
                version: plugin.version.to_string(),
                author: plugin.author.map(|s| s.to_string()),
                description: plugin.description.map(|s| s.to_string()),
                license: plugin.license.map(|s| s.to_string()),
                frontend_path: plugin.frontend_path.map(|s| s.to_string()),
            });

            // Collect on_init callback if present
            if let Some(callback) = plugin.on_init {
                init_callbacks.push(PluginInitCallback {
                    name: plugin.name.to_string(),
                    callback,
                });
            }

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

        let state = AppState::new(self.host, self.port);
        let shared_menus = SharedMenus(Arc::new(menus.clone()));
        let shared_plugins = SharedPlugins(Arc::new(plugins.clone()));

        // Add core routes with vespera
        router = router
            .route("/health", axum::routing::get(health_check))
            .route("/api/menus", axum::routing::get(get_menus))
            .route("/api/plugins", axum::routing::get(get_plugins))
            .layer(Extension(shared_menus))
            .layer(Extension(shared_plugins));

        // Add database connection if configured
        if let Some(ref db) = self.database {
            router = router.layer(Extension(db.clone()));
            tracing::info!("Database connection configured");
        }

        // Add auth routes if auth is configured
        if let Some(ref auth_config) = self.auth_config {
            let auth_config_arc = Arc::new(auth_config.clone());
            router = router.merge(auth_router(auth_config_arc));
            tracing::info!(
                "Auth enabled with superadmin: {}",
                auth_config
                    .superadmin
                    .as_ref()
                    .map(|s| s.username.as_str())
                    .unwrap_or("none")
            );
        }

        // Add static file serving or dev proxy as fallback
        if let Some(dev_proxy_port) = self.dev_proxy_port {
            router = router.merge(dev_proxy_router(dev_proxy_port));
            tracing::info!("Dev proxy enabled -> http://127.0.0.1:{}", dev_proxy_port);
        } else if let Some(static_dir) = self.static_dir {
            router = router.merge(static_router(static_dir));
            tracing::info!("Static file serving enabled");
        }

        // Apply auth middleware if auth is configured
        // This wraps all routes including the fallback (dev proxy/static)
        if let Some(ref mut auth_config) = self.auth_config {
            // Auto-detect public routes by scanning (public) directory
            let public_dir = std::path::Path::new(".yeollin/app/src/app/(public)");
            if public_dir.exists() {
                let routes = scan_routes(public_dir);
                for route in &routes {
                    if !auth_config.public_routes.contains(route) {
                        auth_config.public_routes.push(route.clone());
                    }
                }
                if !routes.is_empty() {
                    tracing::info!(
                        "Auto-detected {} public routes from (public) directory",
                        routes.len()
                    );
                }
            }

            // Auto-detect guest routes by scanning (guest) directory
            let guest_dir = std::path::Path::new(".yeollin/app/src/app/(guest)");
            if guest_dir.exists() {
                let routes = scan_routes(guest_dir);
                for route in &routes {
                    if !auth_config.guest_routes.contains(route) {
                        auth_config.guest_routes.push(route.clone());
                    }
                }
                if !routes.is_empty() {
                    tracing::info!(
                        "Auto-detected {} guest routes from (guest) directory",
                        routes.len()
                    );
                }
            }

            let auth_state = AuthState::new(auth_config.clone());
            router = router.layer(middleware::from_fn_with_state(auth_state, auth_middleware));
            tracing::info!("Auth middleware applied");
        }

        YeollinApp {
            router,
            menus,
            plugins,
            state,
            database: self.database,
            init_callbacks,
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
pub async fn get_menus(Extension(menus): Extension<SharedMenus>) -> Json<Vec<MenuConfig>> {
    Json((*menus.0).clone())
}

/// Get all registered plugins
#[vespera::route(get, path = "/api/plugins", tags = ["system"])]
pub async fn get_plugins(Extension(plugins): Extension<SharedPlugins>) -> Json<Vec<PluginInfo>> {
    // Return plugins without frontend_path (internal info)
    let public_plugins: Vec<PluginInfo> = plugins
        .0
        .iter()
        .map(|p| PluginInfo {
            name: p.name.clone(),
            version: p.version.clone(),
            author: p.author.clone(),
            description: p.description.clone(),
            license: p.license.clone(),
            frontend_path: None,
        })
        .collect();
    Json(public_plugins)
}

/// Scan a route group directory to find routes
/// Works for (public), (guest), or any other route group
fn scan_routes(dir: &std::path::Path) -> Vec<String> {
    let mut routes = Vec::new();

    fn scan_dir(dir: &std::path::Path, base: &str, routes: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                if path.is_dir() {
                    // Skip layout files, recurse into subdirs
                    if !name.starts_with('_') {
                        let new_base = if base.is_empty() {
                            format!("/{}", name)
                        } else {
                            format!("{}/{}", base, name)
                        };
                        scan_dir(&path, &new_base, routes);
                    }
                } else if name.starts_with("page.") {
                    // Found a page file - this is a route
                    let route = if base.is_empty() {
                        "/".to_string()
                    } else {
                        base.to_string()
                    };
                    if !routes.contains(&route) {
                        routes.push(route);
                    }
                }
            }
        }
    }

    scan_dir(dir, "", &mut routes);
    routes
}
