//! Frontend assets handling

use include_dir::{Dir, DirEntry};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use yeollin_core::{MenuConfig, MenuItem};

/// Frontend assets embedded in a plugin
pub struct FrontendAssets {
    /// Embedded directory containing TSX files
    dir: Option<&'static Dir<'static>>,
    /// Auto-generated menu from folder structure
    menu: Option<MenuConfig>,
    /// Plugin name (from root route.ts)
    plugin_name: Option<String>,
}

impl FrontendAssets {
    /// Create frontend assets from an embedded directory
    /// Automatically parses App Router folder structure
    pub fn new(dir: &'static Dir<'static>) -> Self {
        let (plugin_name, menu) = Self::parse_app_router(dir);
        Self {
            dir: Some(dir),
            menu,
            plugin_name,
        }
    }

    /// Create empty frontend assets (for API-only plugins)
    pub fn empty() -> Self {
        Self {
            dir: None,
            menu: None,
            plugin_name: None,
        }
    }

    /// Create frontend assets from a filesystem path (scans at runtime)
    /// This is used by the yeollin_plugin! macro to generate menus from the app/ directory
    pub fn from_path(plugin_name: &str, path: &str) -> Self {
        let (menu, _) = Self::parse_app_router_from_path(plugin_name, path);
        Self {
            dir: None,
            menu,
            plugin_name: Some(plugin_name.to_string()),
        }
    }

    /// Get the embedded directory
    pub fn dir(&self) -> Option<&'static Dir<'static>> {
        self.dir
    }

    /// Get the menu configuration
    pub fn menu(&self) -> Option<&MenuConfig> {
        self.menu.as_ref()
    }

    /// Get plugin name
    pub fn plugin_name(&self) -> Option<&str> {
        self.plugin_name.as_deref()
    }

    /// Check if this plugin has frontend assets
    pub fn has_frontend(&self) -> bool {
        self.dir.is_some()
    }

    /// Get all TSX/TS file paths
    pub fn tsx_files(&self) -> Vec<&'static str> {
        let Some(dir) = self.dir else {
            return vec![];
        };
        Self::collect_tsx_files(dir)
    }

    fn collect_tsx_files(dir: &'static Dir<'static>) -> Vec<&'static str> {
        let mut files = vec![];
        for entry in dir.entries() {
            match entry {
                DirEntry::Dir(d) => {
                    files.extend(Self::collect_tsx_files(d));
                }
                DirEntry::File(f) => {
                    let path = f.path().to_str().unwrap_or("");
                    if path.ends_with(".tsx") || path.ends_with(".ts") {
                        files.push(path);
                    }
                }
            }
        }
        files
    }

    /// Parse App Router folder structure to generate menu
    fn parse_app_router(dir: &'static Dir<'static>) -> (Option<String>, Option<MenuConfig>) {
        // Try to get plugin config from root route.ts
        let plugin_config = Self::parse_route_config(dir);
        let plugin_name = plugin_config
            .as_ref()
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let plugin_label = plugin_config
            .as_ref()
            .and_then(|c| c.get("label"))
            .and_then(|v| v.as_str())
            .unwrap_or("Plugin");

        let plugin_icon = plugin_config
            .as_ref()
            .and_then(|c| c.get("icon"))
            .and_then(|v| v.as_str());

        let plugin_order = plugin_config
            .as_ref()
            .and_then(|c| c.get("order"))
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as i32;

        // Scan for route groups (folders starting with parentheses)
        let mut children = vec![];
        let name = plugin_name.clone().unwrap_or_else(|| "plugin".to_string());
        let base_path = format!("/{}", name);

        for entry in dir.entries() {
            if let DirEntry::Dir(subdir) = entry {
                let dir_name = subdir
                    .path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Check if it's a route group like (example)
                if dir_name.starts_with('(') && dir_name.ends_with(')') {
                    // Scan children of the route group
                    children.extend(Self::scan_routes(subdir, &base_path));
                }
            }
        }

        // Sort children by order
        children.sort_by_key(|item| item.order);

        // Build root menu item
        let root_item = MenuItem {
            id: name.clone(),
            label: plugin_label.to_string(),
            icon: plugin_icon.map(|s| s.to_string()),
            path: format!("/{}", name),
            order: plugin_order,
            children,
        };

        let menu = MenuConfig {
            plugin: name.clone(),
            items: vec![root_item],
        };

        (plugin_name, Some(menu))
    }

    /// Recursively scan routes from a directory
    fn scan_routes(dir: &'static Dir<'static>, base_path: &str) -> Vec<MenuItem> {
        let mut items = vec![];

        for entry in dir.entries() {
            if let DirEntry::Dir(subdir) = entry {
                let dir_name = subdir
                    .path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Skip hidden folders and route groups at this level
                if dir_name.starts_with('.') || dir_name.starts_with('_') {
                    continue;
                }

                // Skip folders that start with ( - they're route groups
                if dir_name.starts_with('(') {
                    continue;
                }

                // Check if this folder has a page.tsx (is a valid route)
                let has_page = subdir.get_file("page.tsx").is_some();
                if !has_page {
                    continue;
                }

                // Try to get route config
                let route_config = Self::parse_route_config(subdir);

                let label = route_config
                    .as_ref()
                    .and_then(|c| c.get("label"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(dir_name);

                let icon = route_config
                    .as_ref()
                    .and_then(|c| c.get("icon"))
                    .and_then(|v| v.as_str());

                let order = route_config
                    .as_ref()
                    .and_then(|c| c.get("order"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(50) as i32;

                let hidden = route_config
                    .as_ref()
                    .and_then(|c| c.get("hidden"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if hidden {
                    continue;
                }

                let path = format!("{}/{}", base_path, dir_name);

                // Recursively scan for nested routes
                let children = Self::scan_routes(subdir, &path);

                items.push(MenuItem {
                    id: dir_name.to_string(),
                    label: label.to_string(),
                    icon: icon.map(|s| s.to_string()),
                    path,
                    order,
                    children,
                });
            }
        }

        items
    }

    /// Parse route.ts file to extract config
    /// Uses simple regex since we can't run JS
    fn parse_route_config(
        dir: &'static Dir<'static>,
    ) -> Option<HashMap<String, serde_json::Value>> {
        let route_file = dir.get_file("route.ts")?;
        let content = route_file.contents_utf8()?;
        Self::parse_route_config_str(content)
    }

    /// Parse route.ts content string to extract config
    fn parse_route_config_str(content: &str) -> Option<HashMap<String, serde_json::Value>> {
        let mut config = HashMap::new();

        // Match: name: "value" or label: "value" etc
        let string_re = regex::Regex::new(r#"(\w+):\s*["']([^"']+)["']"#).ok()?;
        for cap in string_re.captures_iter(content) {
            let key = cap.get(1)?.as_str();
            let value = cap.get(2)?.as_str();
            config.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }

        // Match: order: 100 (numbers)
        let num_re = regex::Regex::new(r"(\w+):\s*(\d+)").ok()?;
        for cap in num_re.captures_iter(content) {
            let key = cap.get(1)?.as_str();
            let value: i64 = cap.get(2)?.as_str().parse().ok()?;
            // Don't override string values
            if !config.contains_key(key) {
                config.insert(key.to_string(), serde_json::Value::Number(value.into()));
            }
        }

        // Match: hidden: true/false
        let bool_re = regex::Regex::new(r"(\w+):\s*(true|false)").ok()?;
        for cap in bool_re.captures_iter(content) {
            let key = cap.get(1)?.as_str();
            let value = cap.get(2)?.as_str() == "true";
            if !config.contains_key(key) {
                config.insert(key.to_string(), serde_json::Value::Bool(value));
            }
        }

        if config.is_empty() {
            None
        } else {
            Some(config)
        }
    }

    /// Parse App Router folder structure from filesystem path to generate menu
    fn parse_app_router_from_path(
        plugin_name: &str,
        path: &str,
    ) -> (Option<MenuConfig>, Option<String>) {
        let dir_path = Path::new(path);
        if !dir_path.exists() || !dir_path.is_dir() {
            return (None, None);
        }

        // Try to get plugin config from root route.ts
        let plugin_config = Self::parse_route_config_from_file(&dir_path.join("route.ts"));

        let plugin_label = plugin_config
            .as_ref()
            .and_then(|c| c.get("label"))
            .and_then(|v| v.as_str())
            .unwrap_or(plugin_name);

        let plugin_icon = plugin_config
            .as_ref()
            .and_then(|c| c.get("icon"))
            .and_then(|v| v.as_str());

        let plugin_order = plugin_config
            .as_ref()
            .and_then(|c| c.get("order"))
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as i32;

        // Scan for route groups (folders starting with parentheses)
        let mut children = vec![];
        let base_path = format!("/{}", plugin_name);

        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                if !entry_path.is_dir() {
                    continue;
                }

                let dir_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Check if it's a route group like (example)
                if dir_name.starts_with('(') && dir_name.ends_with(')') {
                    children.extend(Self::scan_routes_from_path(&entry_path, &base_path));
                }
            }
        }

        // Sort children by order
        children.sort_by_key(|item| item.order);

        // Build root menu item
        let root_item = MenuItem {
            id: plugin_name.to_string(),
            label: plugin_label.to_string(),
            icon: plugin_icon.map(|s| s.to_string()),
            path: format!("/{}", plugin_name),
            order: plugin_order,
            children,
        };

        let menu = MenuConfig {
            plugin: plugin_name.to_string(),
            items: vec![root_item],
        };

        (Some(menu), Some(plugin_name.to_string()))
    }

    /// Recursively scan routes from filesystem
    fn scan_routes_from_path(dir_path: &Path, base_path: &str) -> Vec<MenuItem> {
        let mut items = vec![];

        let Ok(entries) = fs::read_dir(dir_path) else {
            return items;
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }

            let dir_name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip hidden folders and route groups
            if dir_name.starts_with('.') || dir_name.starts_with('_') || dir_name.starts_with('(') {
                continue;
            }

            // Check if this folder has a page.tsx (is a valid route)
            if !entry_path.join("page.tsx").exists() {
                continue;
            }

            // Try to get route config
            let route_config = Self::parse_route_config_from_file(&entry_path.join("route.ts"));

            let label = route_config
                .as_ref()
                .and_then(|c| c.get("label"))
                .and_then(|v| v.as_str())
                .unwrap_or(dir_name);

            let icon = route_config
                .as_ref()
                .and_then(|c| c.get("icon"))
                .and_then(|v| v.as_str());

            let order = route_config
                .as_ref()
                .and_then(|c| c.get("order"))
                .and_then(|v| v.as_i64())
                .unwrap_or(50) as i32;

            let hidden = route_config
                .as_ref()
                .and_then(|c| c.get("hidden"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if hidden {
                continue;
            }

            let path = format!("{}/{}", base_path, dir_name);
            let children = Self::scan_routes_from_path(&entry_path, &path);

            items.push(MenuItem {
                id: dir_name.to_string(),
                label: label.to_string(),
                icon: icon.map(|s| s.to_string()),
                path,
                order,
                children,
            });
        }

        items
    }

    /// Parse route.ts file from filesystem
    fn parse_route_config_from_file(path: &Path) -> Option<HashMap<String, serde_json::Value>> {
        let content = fs::read_to_string(path).ok()?;
        Self::parse_route_config_str(&content)
    }
}
