//! Plugin definition macros

/// Define a Yeollin plugin with minimal boilerplate.
///
/// This macro automatically:
/// - Creates the `metadata()` function with vespera router
/// - Sets up `frontend_path` for prebuild
/// - Uses `CARGO_PKG_VERSION` for version
/// - Uses `CARGO_PKG_LICENSE` for license (if available)
///
/// # Examples
///
/// ## Full usage
///
/// ```rust,ignore
/// mod routes;
///
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     author: "Your Name",
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
///     author: "Your Name",
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
///     author: "Your Name",
///     description: "My awesome plugin",
///     frontend: false,
/// }
/// ```
#[macro_export]
macro_rules! yeollin_plugin {
    // Full: name, author, description, default vespera router
    (
        name: $name:literal,
        author: $author:literal,
        description: $desc:literal $(,)?
    ) => {
        use $crate::{vespera, FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .author($author)
                .description($desc)
                .license(env!("CARGO_PKG_LICENSE"))
                .router(vespera::vespera!())
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Full: name, author, description, custom router
    (
        name: $name:literal,
        author: $author:literal,
        description: $desc:literal,
        router: $router:expr $(,)?
    ) => {
        use $crate::{FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .author($author)
                .description($desc)
                .license(env!("CARGO_PKG_LICENSE"))
                .router($router)
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // API-only: name, author, description, no frontend, default vespera router
    (
        name: $name:literal,
        author: $author:literal,
        description: $desc:literal,
        frontend: false $(,)?
    ) => {
        use $crate::{vespera, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .author($author)
                .description($desc)
                .license(env!("CARGO_PKG_LICENSE"))
                .router(vespera::vespera!())
                .build()
        }
    };

    // API-only: name, author, description, custom router, no frontend
    (
        name: $name:literal,
        author: $author:literal,
        description: $desc:literal,
        router: $router:expr,
        frontend: false $(,)?
    ) => {
        use $crate::PluginMetadata;

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .author($author)
                .description($desc)
                .license(env!("CARGO_PKG_LICENSE"))
                .router($router)
                .build()
        }
    };

    // Legacy: name, description only (no author), default vespera router
    (
        name: $name:literal,
        description: $desc:literal $(,)?
    ) => {
        use $crate::{vespera, FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .description($desc)
                .license(env!("CARGO_PKG_LICENSE"))
                .router(vespera::vespera!())
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Legacy: name, description, custom router (no author)
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
                .license(env!("CARGO_PKG_LICENSE"))
                .router($router)
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Minimal: name only
    (
        name: $name:literal $(,)?
    ) => {
        use $crate::{vespera, FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .license(env!("CARGO_PKG_LICENSE"))
                .router(vespera::vespera!())
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Minimal: name, custom router
    (
        name: $name:literal,
        router: $router:expr $(,)?
    ) => {
        use $crate::{FrontendAssets, PluginMetadata};

        /// Plugin metadata entry point
        pub fn metadata() -> PluginMetadata {
            PluginMetadata::builder($name, env!("CARGO_PKG_VERSION"))
                .license(env!("CARGO_PKG_LICENSE"))
                .router($router)
                .frontend(FrontendAssets::empty())
                .frontend_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../app"))
                .build()
        }
    };

    // Legacy API-only: name, description, no frontend (no author)
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
                .license(env!("CARGO_PKG_LICENSE"))
                .router(vespera::vespera!())
                .build()
        }
    };

    // Legacy API-only: name, description, custom router, no frontend (no author)
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
                .license(env!("CARGO_PKG_LICENSE"))
                .router($router)
                .build()
        }
    };
}
