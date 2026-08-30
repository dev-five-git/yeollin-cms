//! Example Plugin for Yeollin CMS
//!
//! This plugin demonstrates how to create a plugin with both
//! backend API routes and frontend UI components.

mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "example-plugin",
    author: "DevFive",
    description: "Example plugin demonstrating Yeollin CMS plugin architecture",
}

// Re-export types for external use
pub use routes::items::ExampleItem;
