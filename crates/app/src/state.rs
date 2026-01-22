//! Application state

use std::sync::Arc;
use yeollin_core::MenuConfig;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    host: String,
    port: u16,
    menus: Vec<MenuConfig>,
}

impl AppState {
    pub fn new(host: String, port: u16, menus: Vec<MenuConfig>) -> Self {
        Self {
            inner: Arc::new(AppStateInner { host, port, menus }),
        }
    }

    pub fn host(&self) -> &str {
        &self.inner.host
    }

    pub fn port(&self) -> u16 {
        self.inner.port
    }

    pub fn menus(&self) -> &[MenuConfig] {
        &self.inner.menus
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.inner.host, self.inner.port)
    }
}
