//! Procedural macros for Yeollin CMS
//!
//! This crate provides:
//! - `yeollin_plugin!` - Define a plugin with auto-generated export name
//! - `yeollin_app!` - Build an app with automatic plugin registration

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, LitBool, LitStr, Path, Token,
};

/// Convert a kebab-case or snake_case string to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split(|c| c == '-' || c == '_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Convert an identifier (snake_case) to PascalCase
fn ident_to_pascal_case(ident: &Ident) -> Ident {
    format_ident!("{}", to_pascal_case(&ident.to_string()))
}

// ============================================================
// yeollin_plugin! macro
// ============================================================

/// Plugin definition fields
struct PluginDef {
    name: LitStr,
    author: Option<LitStr>,
    description: Option<LitStr>,
    on_init: Option<Expr>,
    frontend: Option<bool>,
}

impl Parse for PluginDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut author: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut on_init: Option<Expr> = None;
        let mut frontend: Option<bool> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "name" => {
                    name = Some(input.parse()?);
                }
                "author" => {
                    author = Some(input.parse()?);
                }
                "description" => {
                    description = Some(input.parse()?);
                }
                "on_init" => {
                    on_init = Some(input.parse()?);
                }
                "frontend" => {
                    let val: LitBool = input.parse()?;
                    frontend = Some(val.value());
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown field: {}", key),
                    ));
                }
            }

            // Optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let name = name.ok_or_else(|| input.error("missing required field: name"))?;

        Ok(PluginDef {
            name,
            author,
            description,
            on_init,
            frontend,
        })
    }
}

/// Define a Yeollin plugin with automatic OpenAPI export.
///
/// This macro automatically:
/// - Converts the plugin name to PascalCase for the export identifier
///   (e.g., "example-plugin" → `ExamplePlugin`)
/// - Generates `vespera::export_app!` for OpenAPI merging
/// - Creates the `metadata()` function
///
/// # Example
///
/// ```rust,ignore
/// mod routes;
///
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     author: "Your Name",
///     description: "My awesome plugin",
/// }
///
/// // Auto-generates: vespera::export_app!(MyPlugin);
/// // In your app, use: merge = [my_plugin::MyPlugin]
/// ```
///
/// ## With on_init callback
///
/// ```rust,ignore
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     author: "Your Name",
///     description: "My awesome plugin",
///     on_init: my_init_fn,
/// }
/// ```
///
/// ## API-only (no frontend)
///
/// ```rust,ignore
/// yeollin_plugin::yeollin_plugin! {
///     name: "my-plugin",
///     author: "Your Name",
///     description: "My awesome plugin",
///     frontend: false,
/// }
/// ```
/// Check if vespertide.json exists in the crate being compiled
fn has_vespertide_json() -> bool {
    std::env::var("CARGO_MANIFEST_DIR")
        .map(|dir| std::path::Path::new(&dir).join("vespertide.json").exists())
        .unwrap_or(false)
}

#[proc_macro]
pub fn yeollin_plugin(input: TokenStream) -> TokenStream {
    let def = parse_macro_input!(input as PluginDef);

    let name_str = def.name.value();
    let export_name = to_pascal_case(&name_str);
    let export_ident = format_ident!("{}", export_name);

    let name_lit = &def.name;
    let has_frontend = def.frontend.unwrap_or(true);

    let author_setter = def.author.as_ref().map(|a| {
        quote! { .author(#a) }
    });

    let description_setter = def.description.as_ref().map(|d| {
        quote! { .description(#d) }
    });

    // Auto-detect vespertide.json and generate on_init if not explicitly provided
    let (on_init_fn, on_init_setter) = if let Some(init) = &def.on_init {
        // User provided explicit on_init
        (quote! {}, quote! { .on_init(#init) })
    } else if has_vespertide_json() {
        // Auto-generate on_init for vespertide migrations
        (
            quote! {
                /// Auto-generated database migration initializer
                async fn __yeollin_auto_on_init(
                    db: yeollin_plugin::DatabaseConnection
                ) -> anyhow::Result<()> {
                    vespertide::vespertide_migration!(&db).await?;
                    Ok(())
                }
            },
            quote! { .on_init(__yeollin_auto_on_init) },
        )
    } else {
        // No on_init needed
        (quote! {}, quote! {})
    };

    let frontend_path_const = if has_frontend {
        quote! {
            const FRONTEND_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/app");
        }
    } else {
        quote! {}
    };

    let frontend_setters = if has_frontend {
        quote! {
            .frontend(yeollin_plugin::FrontendAssets::from_path(#name_lit, FRONTEND_PATH))
            .frontend_path(FRONTEND_PATH)
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        // Auto-generate export_app with PascalCase name derived from plugin name
        yeollin_plugin::vespera::export_app!(#export_ident);

        #frontend_path_const

        #on_init_fn

        /// Plugin metadata entry point
        pub fn metadata() -> yeollin_plugin::PluginMetadata {
            yeollin_plugin::PluginMetadata::builder(#name_lit, env!("CARGO_PKG_VERSION"))
                #author_setter
                #description_setter
                .license(env!("CARGO_PKG_LICENSE"))
                .router(yeollin_plugin::vespera::axum::Router::new())
                #on_init_setter
                #frontend_setters
                .build()
        }
    };

    TokenStream::from(expanded)
}

// ============================================================
// yeollin_app! macro
// ============================================================

/// App definition fields
struct AppDef {
    plugins: Vec<Path>,
    openapi: Option<LitStr>,
    title: Option<LitStr>,
    version: Option<LitStr>,
    docs_url: Option<LitStr>,
    redoc_url: Option<LitStr>,
}

impl Parse for AppDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut plugins: Vec<Path> = vec![];
        let mut openapi: Option<LitStr> = None;
        let mut title: Option<LitStr> = None;
        let mut version: Option<LitStr> = None;
        let mut docs_url: Option<LitStr> = None;
        let mut redoc_url: Option<LitStr> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "plugins" => {
                    let content;
                    bracketed!(content in input);
                    let paths: Punctuated<Path, Token![,]> =
                        content.parse_terminated(Path::parse, Token![,])?;
                    plugins = paths.into_iter().collect();
                }
                "openapi" => {
                    openapi = Some(input.parse()?);
                }
                "title" => {
                    title = Some(input.parse()?);
                }
                "version" => {
                    version = Some(input.parse()?);
                }
                "docs_url" => {
                    docs_url = Some(input.parse()?);
                }
                "redoc_url" => {
                    redoc_url = Some(input.parse()?);
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown field: {}", key),
                    ));
                }
            }

            // Optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(AppDef {
            plugins,
            openapi,
            title,
            version,
            docs_url,
            redoc_url,
        })
    }
}

/// Build a Yeollin app with automatic plugin registration and OpenAPI merging.
///
/// This macro automatically:
/// - Registers all plugins via `register_plugin()`
/// - Merges all plugin routes via `vespera!(merge = [...])`
/// - Configures OpenAPI documentation
///
/// # Example
///
/// ```rust,ignore
/// let app = yeollin::yeollin_app! {
///     plugins: [example_plugin, example_memo_plugin],
///     openapi: "openapi.json",
///     title: "Example CMS API",
///     version: "1.0.0",
///     docs_url: "/docs",
///     redoc_url: "/redoc",
/// };
///
/// // Then configure and build:
/// app.host("0.0.0.0")
///    .port(3001)
///    .with_auth(auth_config)
///    .with_database(db)
///    .build()
///    .run()
///    .await
/// ```
#[proc_macro]
pub fn yeollin_app(input: TokenStream) -> TokenStream {
    let def = parse_macro_input!(input as AppDef);

    // Generate register_plugin calls
    let register_plugins = def.plugins.iter().map(|plugin| {
        quote! {
            .register_plugin(#plugin::metadata())
        }
    });

    // Generate merge list with PascalCase export names
    let merge_exports: Vec<_> = def
        .plugins
        .iter()
        .map(|plugin| {
            // Get the last segment of the path (e.g., example_plugin from crate::example_plugin)
            let last_segment = plugin.segments.last().unwrap();
            let export_ident = ident_to_pascal_case(&last_segment.ident);
            quote! { #plugin::#export_ident }
        })
        .collect();

    // Generate vespera config
    let openapi_config = def.openapi.as_ref().map(|o| quote! { openapi = #o, });
    let title_config = def.title.as_ref().map(|t| quote! { title = #t, });
    let version_config = def.version.as_ref().map(|v| quote! { version = #v, });
    let docs_url_config = def.docs_url.as_ref().map(|d| quote! { docs_url = #d, });
    let redoc_url_config = def.redoc_url.as_ref().map(|r| quote! { redoc_url = #r, });

    let expanded = quote! {
        yeollin::app()
            #(#register_plugins)*
            .merge(yeollin::vespera::vespera!(
                #openapi_config
                #title_config
                #version_config
                #docs_url_config
                #redoc_url_config
                merge = [#(#merge_exports),*]
            ).with_state(()))
    };

    TokenStream::from(expanded)
}
