//! Frontend assets contributed by a plugin.
//!
//! Routes are compiled from the plugin's App Router tree by
//! [`yeollin_core::compile_route_manifest`], so menus and access rules come from
//! one typed source instead of being re-derived by each consumer.

use std::path::Path;

use yeollin_core::{
    build_menu, compile_route_manifest, MenuConfig, RouteDiagnostic, RouteEntry, RouteSource,
};

/// Frontend routes and navigation contributed by a plugin.
pub struct FrontendAssets {
    routes: Vec<RouteEntry>,
    menu: Option<MenuConfig>,
    plugin_name: Option<String>,
}

impl FrontendAssets {
    /// Create empty frontend assets (for API-only plugins)
    pub fn empty() -> Self {
        Self {
            routes: vec![],
            menu: None,
            plugin_name: None,
        }
    }

    /// Resolve the plugin's routes, preferring the source tree over the routes
    /// baked in at compile time.
    ///
    /// During development `path` exists, so edits to `route.meta.json` take
    /// effect on restart without a rebuild. In a deployed binary that path
    /// belongs to the build machine and is gone, and `embedded` carries the same
    /// routes so access rules and menus survive.
    ///
    /// # Panics
    /// Panics when the source tree contains invalid route metadata. Registration
    /// happens before any request is served, and a broken access rule must stop
    /// startup rather than silently fall back to a default.
    pub fn compile(plugin_name: &str, path: &str, embedded: &str) -> Self {
        let dir = Path::new(path);

        let routes = if dir.is_dir() {
            match compile_route_manifest(&[RouteSource::plugin(plugin_name, dir)]) {
                Ok(manifest) => manifest.routes,
                Err(diagnostics) => panic!("{}", describe(plugin_name, &diagnostics)),
            }
        } else {
            serde_json::from_str(embedded).unwrap_or_default()
        };

        let menu = build_menu(&routes, plugin_name);

        Self {
            routes,
            menu,
            plugin_name: Some(plugin_name.to_string()),
        }
    }

    /// Get the menu configuration
    pub fn menu(&self) -> Option<&MenuConfig> {
        self.menu.as_ref()
    }

    /// Get plugin name
    pub fn plugin_name(&self) -> Option<&str> {
        self.plugin_name.as_deref()
    }

    /// Routes this plugin contributes, in manifest order
    pub fn routes(&self) -> &[RouteEntry] {
        &self.routes
    }

    /// Whether this plugin contributes any page routes
    pub fn has_frontend(&self) -> bool {
        !self.routes.is_empty()
    }
}

fn describe(plugin_name: &str, diagnostics: &[RouteDiagnostic]) -> String {
    let details = diagnostics
        .iter()
        .map(|diagnostic| format!("\n  {diagnostic}"))
        .collect::<String>();
    format!("plugin `{plugin_name}` has invalid route metadata:{details}")
}
