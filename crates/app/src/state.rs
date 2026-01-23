//! Application state

use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    host: String,
    port: u16,
}

impl AppState {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            inner: Arc::new(AppStateInner { host, port }),
        }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.inner.host, self.inner.port)
    }
}
