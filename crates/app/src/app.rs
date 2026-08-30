//! Yeollin application builder

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
use yeollin_core::{
    compile_route_manifest, EventBus, ExportEnvelope, MenuConfig, PluginInfo, RouteAccess,
    RouteEntry, RouteSource, SettingsRegistration, SettingsStore, SubscriberRegistration,
    EXPORT_ENV_VAR, EXPORT_SCHEMA_VERSION,
};
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
    /// Retained so `run` can reject an unsigned-capable config before serving
    auth_config: Option<AuthConfig>,
    /// Compiled page routes, exported for prebuild
    routes: Vec<RouteEntry>,
    /// Connected lazily by `run`, so export mode touches no database
    database_url: Option<String>,
    /// Settings contracts are retained until a database exists at runtime.
    settings_registrations: Vec<SettingsRegistration>,
    /// Subscribers are bound to the event bus after the database is connected.
    subscriber_registrations: Vec<SubscriberRegistration>,
}

impl YeollinApp {
    /// Run the CMS server.
    ///
    /// When [`EXPORT_ENV_VAR`] is set the process instead writes one
    /// [`ExportEnvelope`] to stdout and exits. That branch is deliberately the
    /// very first thing `run` does: prebuild invokes the binary purely to read
    /// metadata, so it must not connect to a database, run plugin
    /// initialisation, or require deployment secrets.
    pub async fn run(mut self) -> anyhow::Result<()> {
        if std::env::var_os(EXPORT_ENV_VAR).is_some() {
            let envelope = ExportEnvelope {
                schema_version: EXPORT_SCHEMA_VERSION,
                plugins: self.plugins,
                menus: self.menus,
                routes: self.routes,
            };
            // Exactly one document, and nothing else, so the reader never has to
            // guess where the payload starts.
            println!("{}", serde_json::to_string(&envelope)?);
            return Ok(());
        }

        // Fail before any traffic is served rather than issuing forgeable tokens.
        // Deliberately placed after the export branch, which never signs anything
        // and runs during prebuild without deployment secrets present.
        if let Some(auth_config) = &self.auth_config {
            auth_config.validate()?;
        }

        if self.database.is_none() {
            if let Some(url) = self.database_url.take() {
                tracing::info!("Connecting to database");
                let db = sea_orm::Database::connect(url).await?;
                self.router = self.router.layer(Extension(db.clone()));
                self.database = Some(db);
            }
        }

        let event_bus = if let Some(db) = self.database.clone() {
            yeollin_core::migrate_core(&db).await?;

            if !self.settings_registrations.is_empty() {
                let settings = SettingsStore::new(db.clone(), self.settings_registrations)?;
                settings.initialize().await?;
                self.router = self.router.layer(Extension(settings));
            }

            let events = EventBus::new(db, self.subscriber_registrations)?;
            self.router = self.router.layer(Extension(events.clone()));
            Some(events)
        } else {
            if !self.settings_registrations.is_empty() {
                anyhow::bail!("plugins register settings but no database is configured");
            }
            if !self.subscriber_registrations.is_empty() {
                anyhow::bail!("plugins register event subscribers but no database is configured");
            }
            None
        };

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

        let drainer = event_bus.as_ref().map(EventBus::start_drainer);
        let server = Server::new(self.router, self.state);
        let result = server.run().await;
        if let Some(drainer) = drainer {
            drainer.abort();
        }
        result
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
    app_frontend: Option<(&'static str, &'static str)>,
    database_url: Option<String>,
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
            app_frontend: None,
            database_url: None,
        }
    }

    /// Connect to the database when the server starts rather than at build time.
    ///
    /// Preferred over [`Self::with_database`]: metadata-export runs never open a
    /// connection, so prebuild cannot create or migrate a database as a side
    /// effect of reading plugin information.
    pub fn with_database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Register the host application's own `app/` directory so its route
    /// metadata contributes access rules, exactly like a plugin's.
    ///
    /// `embedded` holds the same routes compiled at build time and is used when
    /// `path` is absent, which is the case for a binary running away from the
    /// machine that built it.
    pub fn app_frontend(mut self, path: &'static str, embedded: &'static str) -> Self {
        self.app_frontend = Some((path, embedded));
        self
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

        // Vite dev paths are only reachable while the dev proxy is live, so they
        // are exempted from auth only then.
        let dev_mode = self.dev_proxy_port.is_some();
        if let Some(ref mut auth_config) = self.auth_config {
            auth_config.dev_mode = dev_mode;
        }

        let mut router = Router::new();
        let mut menus = vec![];
        let mut plugins = vec![];
        let mut init_callbacks = vec![];
        let mut settings_registrations = vec![];
        let mut subscriber_registrations = vec![];
        let mut page_routes: Vec<RouteEntry> = vec![];

        if let Some((path, embedded)) = self.app_frontend {
            if std::path::Path::new(path).is_dir() {
                match compile_route_manifest(&[RouteSource::app(path)]) {
                    Ok(manifest) => page_routes.extend(manifest.routes),
                    Err(diagnostics) => {
                        let details = diagnostics
                            .iter()
                            .map(|diagnostic| format!("\n  {diagnostic}"))
                            .collect::<String>();
                        panic!("application has invalid route metadata:{details}");
                    }
                }
            } else {
                page_routes
                    .extend(serde_json::from_str::<Vec<RouteEntry>>(embedded).unwrap_or_default());
            }
        }

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
                subscribers = plugin.subscribers.len(),
                "Merging plugin router"
            );

            // Collect plugin info for export
            let settings_info = plugin
                .settings
                .as_ref()
                .map(SettingsRegistration::export_info);
            plugins.push(PluginInfo {
                name: plugin.name.to_string(),
                version: plugin.version.to_string(),
                author: plugin.author.map(|s| s.to_string()),
                description: plugin.description.map(|s| s.to_string()),
                license: plugin.license.map(|s| s.to_string()),
                frontend_path: plugin.frontend_path.map(|s| s.to_string()),
                settings: settings_info,
            });

            if let Some(settings) = plugin.settings {
                settings_registrations.push(settings);
            }

            subscriber_registrations.extend(
                plugin
                    .subscribers
                    .into_iter()
                    .map(|subscriber| subscriber.for_plugin(plugin.name)),
            );

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

            page_routes.extend(plugin.frontend.routes().iter().cloned());
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

        // Publish the auth config so plugins can sign and verify tokens. The
        // framework itself owns no credential store: login lives in a plugin.
        if let Some(ref auth_config) = self.auth_config {
            router = router.layer(Extension(Arc::new(auth_config.clone())));
            tracing::info!("Auth configured");
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
            // Access comes from compiled route metadata only. Directory names such
            // as `(public)` organise files and grant nothing, so a route that
            // declares no access rule stays authenticated.
            let mut public = 0usize;
            let mut guest = 0usize;
            for route in &page_routes {
                let (bucket, counter) = match route.access {
                    RouteAccess::Public => (&mut auth_config.public_routes, &mut public),
                    RouteAccess::Guest => (&mut auth_config.guest_routes, &mut guest),
                    RouteAccess::Authenticated => continue,
                };
                if !bucket.contains(&route.path) {
                    bucket.push(route.path.clone());
                    *counter += 1;
                }
            }
            tracing::info!(public, guest, "Applied route access rules from manifest");

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
            auth_config: self.auth_config,
            routes: page_routes,
            database_url: self.database_url,
            settings_registrations,
            subscriber_registrations,
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
            settings: p.settings.clone(),
        })
        .collect();
    Json(public_plugins)
}
