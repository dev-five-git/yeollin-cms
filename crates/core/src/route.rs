//! Route metadata sidecars and the manifest compiled from them.
//!
//! Page routes are discovered from the App Router directory tree, but nothing
//! security-relevant is inferred from directory *names*. Access rules come only
//! from an explicit [`ROUTE_META_FILENAME`] sidecar, and anything undeclared
//! stays [`RouteAccess::Authenticated`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vespera::Schema;

use crate::menu::{MenuConfig, MenuItem};

/// Per-route metadata sidecar, placed next to the `page.tsx` it describes.
pub const ROUTE_META_FILENAME: &str = "route.meta.json";

/// Marker file that makes a directory a routable page.
pub const PAGE_FILENAME: &str = "page.tsx";

/// Version of the compiled manifest format.
pub const ROUTE_MANIFEST_SCHEMA_VERSION: u32 = 1;

const DEFAULT_ORDER: i32 = 50;

/// Who may reach a page route.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub enum RouteAccess {
    /// Requires a valid session. Applied to every route that does not say otherwise.
    #[default]
    Authenticated,
    /// Reachable regardless of session state.
    Public,
    /// Reachable only *without* a session; signed-in visitors are redirected away.
    Guest,
}

/// Deserialized contents of a [`ROUTE_META_FILENAME`] file.
///
/// Unknown fields are rejected: a misspelled `acess` key must fail the build
/// rather than silently leave the route on its default access rule.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteMeta {
    pub label: Option<String>,
    pub icon: Option<String>,
    pub order: Option<i32>,
    #[serde(default)]
    pub access: RouteAccess,
    pub menu: Option<bool>,
}

/// One compiled route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct RouteEntry {
    pub path: String,
    /// Owning plugin, or `None` for routes belonging to the host application.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub order: i32,
    pub access: RouteAccess,
    pub menu: bool,
}

/// The compiled, deterministic route table for one application build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct RouteManifest {
    pub schema_version: u32,
    pub routes: Vec<RouteEntry>,
}

impl RouteManifest {
    pub fn paths_with_access(&self, access: RouteAccess) -> Vec<String> {
        self.routes
            .iter()
            .filter(|route| route.access == access)
            .map(|route| route.path.clone())
            .collect()
    }
}

/// Nest a plugin's menu-visible routes under its root entry.
///
/// Routes are attached to the deepest already-emitted ancestor, so a page whose
/// parent directory has no `page.tsx` still appears rather than being dropped.
pub fn build_menu(routes: &[RouteEntry], plugin: &str) -> Option<MenuConfig> {
    let root_path = format!("/{plugin}");
    let mut owned: Vec<&RouteEntry> = routes
        .iter()
        .filter(|route| route.menu && route.path.starts_with(&root_path))
        .collect();
    owned.sort_by(|a, b| {
        a.path
            .matches('/')
            .count()
            .cmp(&b.path.matches('/').count())
            .then_with(|| a.order.cmp(&b.order))
            .then_with(|| a.path.cmp(&b.path))
    });

    let root_entry = owned.iter().find(|route| route.path == root_path)?;
    let mut root = menu_item(root_entry);

    for route in owned.iter().filter(|route| route.path != root_path) {
        let item = menu_item(route);
        attach(&mut root, &route.path, item);
    }

    sort_children(&mut root);

    Some(MenuConfig {
        plugin: plugin.to_string(),
        items: vec![root],
    })
}

fn menu_item(route: &RouteEntry) -> MenuItem {
    MenuItem {
        id: route
            .path
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or(&route.path)
            .to_string(),
        label: route.label.clone(),
        icon: route.icon.clone(),
        path: route.path.clone(),
        order: route.order,
        children: vec![],
    }
}

fn attach(parent: &mut MenuItem, path: &str, item: MenuItem) {
    if let Some(child) = parent
        .children
        .iter_mut()
        .find(|child| path.starts_with(&format!("{}/", child.path)))
    {
        attach(child, path, item);
        return;
    }
    parent.children.push(item);
}

fn sort_children(item: &mut MenuItem) {
    item.children
        .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.path.cmp(&b.path)));
    for child in &mut item.children {
        sort_children(child);
    }
}

/// A problem that must fail the build rather than degrade into a default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDiagnostic {
    pub source: PathBuf,
    pub message: String,
}

impl std::fmt::Display for RouteDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.source.display(), self.message)
    }
}

/// One frontend tree to compile: the host app, or a plugin's `app/` directory.
#[derive(Debug, Clone)]
pub struct RouteSource {
    /// Plugin name, or `None` for the host application.
    pub plugin: Option<String>,
    pub app_dir: PathBuf,
}

impl RouteSource {
    pub fn app(app_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugin: None,
            app_dir: app_dir.into(),
        }
    }

    pub fn plugin(name: impl Into<String>, app_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugin: Some(name.into()),
            app_dir: app_dir.into(),
        }
    }

    fn url_prefix(&self) -> String {
        match &self.plugin {
            Some(name) => format!("/{name}"),
            None => String::new(),
        }
    }
}

/// Compile every source tree into a single deterministic manifest.
///
/// Returns *all* diagnostics rather than the first, so one build reports every
/// broken sidecar instead of surfacing them one run at a time.
pub fn compile_route_manifest(sources: &[RouteSource]) -> Result<RouteManifest, Vec<RouteDiagnostic>> {
    let mut routes: BTreeMap<String, RouteEntry> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for source in sources {
        if !source.app_dir.is_dir() {
            continue;
        }
        collect_routes(
            &source.app_dir,
            &source.url_prefix(),
            source,
            &mut routes,
            &mut diagnostics,
        );
    }

    if !diagnostics.is_empty() {
        diagnostics.sort_by(|a, b| (&a.source, &a.message).cmp(&(&b.source, &b.message)));
        return Err(diagnostics);
    }

    let mut routes: Vec<RouteEntry> = routes.into_values().collect();
    routes.sort_by(|a, b| {
        a.order
            .cmp(&b.order)
            .then_with(|| a.plugin.cmp(&b.plugin))
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(RouteManifest {
        schema_version: ROUTE_MANIFEST_SCHEMA_VERSION,
        routes,
    })
}

fn collect_routes(
    dir: &Path,
    url_prefix: &str,
    source: &RouteSource,
    routes: &mut BTreeMap<String, RouteEntry>,
    diagnostics: &mut Vec<RouteDiagnostic>,
) {
    if dir.join(PAGE_FILENAME).is_file() {
        emit_route(dir, url_prefix, source, routes, diagnostics);
    }

    let Ok(entries) = fs::read_dir(dir) else {
        diagnostics.push(RouteDiagnostic {
            source: dir.to_path_buf(),
            message: "directory could not be read".to_string(),
        });
        return;
    };

    let mut children: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    // Directory iteration order is filesystem-dependent; sort so that diagnostics
    // and collision reports come out identically on every machine.
    children.sort();

    for child in children {
        let Some(name) = child.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }

        // Route groups organise files without contributing a URL segment.
        if name.starts_with('(') && name.ends_with(')') {
            collect_routes(&child, url_prefix, source, routes, diagnostics);
            continue;
        }

        collect_routes(
            &child,
            &format!("{url_prefix}/{name}"),
            source,
            routes,
            diagnostics,
        );
    }
}

fn emit_route(
    dir: &Path,
    url_prefix: &str,
    source: &RouteSource,
    routes: &mut BTreeMap<String, RouteEntry>,
    diagnostics: &mut Vec<RouteDiagnostic>,
) {
    let path = if url_prefix.is_empty() {
        "/".to_string()
    } else {
        url_prefix.to_string()
    };

    if path.contains("..") {
        diagnostics.push(RouteDiagnostic {
            source: dir.to_path_buf(),
            message: format!("route path `{path}` contains a `..` segment"),
        });
        return;
    }

    let meta_path = dir.join(ROUTE_META_FILENAME);
    let meta = match read_route_meta(&meta_path) {
        Ok(meta) => meta,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };

    let is_dynamic = path.contains('[');
    if is_dynamic && meta.menu == Some(true) {
        diagnostics.push(RouteDiagnostic {
            source: meta_path,
            message: format!("route `{path}` has a dynamic segment and cannot be a menu entry"),
        });
        return;
    }

    let default_label = path
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .map(str::to_string)
        .or_else(|| source.plugin.clone())
        .unwrap_or_else(|| "home".to_string());

    let entry = RouteEntry {
        path: path.clone(),
        plugin: source.plugin.clone(),
        label: meta.label.unwrap_or(default_label),
        icon: meta.icon,
        order: meta.order.unwrap_or(DEFAULT_ORDER),
        access: meta.access,
        menu: meta.menu.unwrap_or(!is_dynamic),
    };

    if let Some(existing) = routes.get(&path) {
        diagnostics.push(RouteDiagnostic {
            source: dir.to_path_buf(),
            message: format!(
                "route `{path}` is already defined by {}",
                describe_owner(&existing.plugin)
            ),
        });
        return;
    }

    routes.insert(path, entry);
}

fn read_route_meta(meta_path: &Path) -> Result<RouteMeta, RouteDiagnostic> {
    if !meta_path.is_file() {
        return Ok(RouteMeta::default());
    }

    let raw = fs::read_to_string(meta_path).map_err(|error| RouteDiagnostic {
        source: meta_path.to_path_buf(),
        message: format!("could not be read: {error}"),
    })?;

    serde_json::from_str(&raw).map_err(|error| RouteDiagnostic {
        source: meta_path.to_path_buf(),
        message: format!("is not valid route metadata: {error}"),
    })
}

fn describe_owner(plugin: &Option<String>) -> String {
    match plugin {
        Some(name) => format!("plugin `{name}`"),
        None => "the application".to_string(),
    }
}
