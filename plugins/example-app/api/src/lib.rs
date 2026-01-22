//! Example App Built-in Plugin
//!
//! This is the dashboard plugin built into the example-app.
//! Demonstrates how an app can have its own built-in plugin alongside external ones.

mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "dashboard",
    description: "Built-in dashboard for Example CMS",
}

// Re-export types
pub use routes::api::dashboard::DashboardStats;
