//! Plugin definition macros

/// Define a Yeollin plugin with minimal boilerplate.
///
/// This macro automatically:
/// - Creates the `metadata()` function with vespera router
/// - Sets up `frontend_path` for prebuild
///
/// # Examples
///
/// ## Basic usage (uses vespera::vespera!() for routes)
///
/// ```rust,ignore
/// mod routes;
///
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     description: "My awesome plugin",
/// }
/// ```
///
/// ## With custom router
///
/// ```rust,ignore
/// mod routes;
///
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     description: "My awesome plugin",
///     router: custom_router(),
/// }
/// ```
///
/// ## API-only plugin (no frontend)
///
/// ```rust,ignore
/// mod routes;
///
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     description: "My awesome plugin",
///     frontend: false,
/// }
/// ```
#[macro_export]
macro_rules! yeollin_plugin {
    // With description, default vespera router
    (
        name: $name:literal,
        description: $desc:literal $(,)?
    ) => {
        use $crate::{vespera, FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .description($desc)
                .router(vespera::vespera!())
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // With description and custom router
    (
        name: $name:literal,
        description: $desc:literal,
        router: $router:expr $(,)?
    ) => {
        use $crate::{FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .description($desc)
                .router($router)
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Without description, default vespera router
    (
        name: $name:literal $(,)?
    ) => {
        use $crate::{vespera, FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .router(vespera::vespera!())
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Without description, custom router
    (
        name: $name:literal,
        router: $router:expr $(,)?
    ) => {
        use $crate::{FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .router($router)
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // API-only plugin with description (no frontend), default vespera router
    (
        name: $name:literal,
        description: $desc:literal,
        frontend: false $(,)?
    ) => {
        use $crate::{vespera, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .description($desc)
                .router(vespera::vespera!())
                .build()
        }
    };

    // API-only plugin with description and custom router (no frontend)
    (
        name: $name:literal,
        description: $desc:literal,
        router: $router:expr,
        frontend: false $(,)?
    ) => {
        use $crate::PluginMetadata;

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .description($desc)
                .router($router)
                .build()
        }
    };
}
