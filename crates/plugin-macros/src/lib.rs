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
    Expr, Ident, LitBool, LitInt, LitStr, Path, Token, Type,
};

/// Convert a kebab-case or snake_case string to PascalCase
fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_'])
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
// yeollin_content_collection! macro
// ============================================================

struct ContentCollectionDef {
    module: Ident,
    name: LitStr,
    label: LitStr,
    fields: Type,
    order: LitInt,
}

impl Parse for ContentCollectionDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut module = None;
        let mut name = None;
        let mut label = None;
        let mut fields = None;
        let mut order = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "module" => module = Some(input.parse()?),
                "name" => name = Some(input.parse()?),
                "label" => label = Some(input.parse()?),
                "fields" => fields = Some(input.parse()?),
                "order" => order = Some(input.parse()?),
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown content collection field: {key}"),
                    ));
                }
            }
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            module: module.ok_or_else(|| input.error("missing required field: module"))?,
            name: name.ok_or_else(|| input.error("missing required field: name"))?,
            label: label.ok_or_else(|| input.error("missing required field: label"))?,
            fields: fields.ok_or_else(|| input.error("missing required field: fields"))?,
            order: order.ok_or_else(|| input.error("missing required field: order"))?,
        })
    }
}

fn validate_collection_name(name: &LitStr) -> syn::Result<()> {
    let value = name.value();
    let valid = !value.is_empty()
        && value.len() <= 64
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(syn::Error::new(
            name.span(),
            "content collection names must be 1-64 lowercase kebab-case characters",
        ))
    }
}

/// Declare one compile-time typed content collection.
///
/// The generated module owns concrete request and response types plus the full
/// draft/publish CRUD surface, so Vespera can describe every field in OpenAPI.
#[proc_macro]
pub fn yeollin_content_collection(input: TokenStream) -> TokenStream {
    let def = parse_macro_input!(input as ContentCollectionDef);
    if let Err(error) = validate_collection_name(&def.name) {
        return TokenStream::from(error.to_compile_error());
    }
    if def.label.value().trim().is_empty() {
        return TokenStream::from(
            syn::Error::new(
                def.label.span(),
                "content collection label must not be empty",
            )
            .to_compile_error(),
        );
    }
    let order = match def.order.base10_parse::<i32>() {
        Ok(order) => order,
        Err(error) => {
            return TokenStream::from(syn::Error::new(def.order.span(), error).to_compile_error())
        }
    };

    let module = def.module;
    let name = def.name;
    let label = def.label;
    let fields = def.fields;
    let type_prefix = to_pascal_case(&name.value());
    let response = format_ident!("{type_prefix}ContentResponse");
    let list_response = format_ident!("List{type_prefix}ContentResponse");
    let list_query = format_ident!("List{type_prefix}ContentQuery");
    let published_query = format_ident!("Published{type_prefix}ContentQuery");
    let create_request = format_ident!("Create{type_prefix}ContentRequest");
    let update_request = format_ident!("Update{type_prefix}ContentRequest");
    let delete_response = format_ident!("Delete{type_prefix}ContentResponse");
    let collection_path = LitStr::new(&format!("/{}", name.value()), name.span());
    let item_path = LitStr::new(&format!("/{}/{{id}}", name.value()), name.span());
    let publish_path = LitStr::new(&format!("/{}/{{id}}/publish", name.value()), name.span());
    let unpublish_path = LitStr::new(&format!("/{}/{{id}}/unpublish", name.value()), name.span());
    let published_path = LitStr::new(&format!("/{}/published", name.value()), name.span());

    TokenStream::from(quote! {
        pub mod #module {
            #[derive(Clone, Debug, yeollin_plugin::serde::Serialize, yeollin_plugin::vespera::Schema)]
            #[serde(rename_all = "camelCase")]
            pub struct #response {
                pub id: String,
                pub collection: String,
                pub title: String,
                pub slug: String,
                pub status: yeollin_plugin::ContentStatus,
                pub author: String,
                pub fields: #fields,
                pub created_at: String,
                pub updated_at: String,
                pub published_at: Option<String>,
            }

            impl From<yeollin_plugin::ContentRecord<#fields>> for #response {
                fn from(record: yeollin_plugin::ContentRecord<#fields>) -> Self {
                    Self {
                        id: record.id,
                        collection: record.collection,
                        title: record.title,
                        slug: record.slug,
                        status: record.status,
                        author: record.author,
                        fields: record.fields,
                        created_at: record.created_at,
                        updated_at: record.updated_at,
                        published_at: record.published_at,
                    }
                }
            }

            #[derive(Debug, yeollin_plugin::serde::Serialize, yeollin_plugin::vespera::Schema)]
            #[serde(rename_all = "camelCase")]
            pub struct #list_response {
                pub entries: Vec<#response>,
                pub total: u64,
                pub page: u64,
                pub page_size: u64,
            }

            impl From<yeollin_plugin::ContentPage<#fields>> for #list_response {
                fn from(page: yeollin_plugin::ContentPage<#fields>) -> Self {
                    Self {
                        entries: page.entries.into_iter().map(#response::from).collect(),
                        total: page.total,
                        page: page.page,
                        page_size: page.page_size,
                    }
                }
            }

            #[derive(Default, Debug, yeollin_plugin::serde::Deserialize, yeollin_plugin::vespera::Schema)]
            #[serde(rename_all = "camelCase")]
            pub struct #list_query {
                pub page: Option<u64>,
                pub page_size: Option<u64>,
                pub status: Option<yeollin_plugin::ContentStatus>,
            }

            #[derive(Debug, yeollin_plugin::serde::Deserialize, yeollin_plugin::vespera::Schema)]
            pub struct #published_query {
                pub slug: String,
            }

            #[derive(Debug, yeollin_plugin::serde::Deserialize, yeollin_plugin::vespera::Schema)]
            pub struct #create_request {
                pub title: String,
                pub slug: String,
                pub fields: #fields,
            }

            #[derive(Debug, yeollin_plugin::serde::Deserialize, yeollin_plugin::vespera::Schema)]
            pub struct #update_request {
                pub title: Option<String>,
                pub slug: Option<String>,
                pub fields: Option<#fields>,
            }

            #[derive(Debug, yeollin_plugin::serde::Serialize, yeollin_plugin::vespera::Schema)]
            #[serde(rename_all = "camelCase")]
            pub struct #delete_response {
                pub success: bool,
                pub deleted_id: String,
            }

            pub fn registration() -> yeollin_plugin::ContentCollection {
                let registration = yeollin_plugin::ContentCollectionRegistration::new::<#fields>(
                        #name,
                        #label,
                        #order,
                        yeollin_plugin::serde_json::to_value(
                            yeollin_plugin::vespera::schema!(#fields)
                        ).expect("Vespera content schema must serialize"),
                    );
                let router = yeollin_plugin::vespera::axum::Router::new()
                    .route(
                        #collection_path,
                        yeollin_plugin::vespera::axum::routing::get(list_content)
                            .post(create_content),
                    )
                    .route(
                        #published_path,
                        yeollin_plugin::vespera::axum::routing::get(published_content),
                    )
                    .route(
                        #item_path,
                        yeollin_plugin::vespera::axum::routing::get(get_content)
                            .put(update_content)
                            .delete(delete_content),
                    )
                    .route(
                        #publish_path,
                        yeollin_plugin::vespera::axum::routing::post(publish_content),
                    )
                    .route(
                        #unpublish_path,
                        yeollin_plugin::vespera::axum::routing::post(unpublish_content),
                    );
                yeollin_plugin::ContentCollection::new(registration, router)
            }

            #[yeollin_plugin::vespera::route(get, path = #collection_path, tags = [#name])]
            pub async fn list_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Query(query):
                    yeollin_plugin::vespera::axum::extract::Query<#list_query>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#list_response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let page = query.page.unwrap_or(1);
                let page_size = query.page_size.unwrap_or(yeollin_plugin::DEFAULT_CONTENT_PAGE_SIZE);
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .list(page, page_size, query.status)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(get, path = #item_path, tags = [#name])]
            pub async fn get_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Path(id):
                    yeollin_plugin::vespera::axum::extract::Path<String>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .get(&id)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(post, path = #collection_path, tags = [#name])]
            pub async fn create_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(events):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::EventBus>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::Json(request):
                    yeollin_plugin::vespera::axum::Json<#create_request>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .create(
                        &events,
                        &current.sub,
                        yeollin_plugin::NewContent {
                            title: request.title,
                            slug: request.slug,
                            fields: request.fields,
                        },
                    )
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(put, path = #item_path, tags = [#name])]
            pub async fn update_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(events):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::EventBus>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Path(id):
                    yeollin_plugin::vespera::axum::extract::Path<String>,
                yeollin_plugin::vespera::axum::Json(request):
                    yeollin_plugin::vespera::axum::Json<#update_request>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .update(
                        &events,
                        &current.sub,
                        &id,
                        yeollin_plugin::ContentPatch {
                            title: request.title,
                            slug: request.slug,
                            fields: request.fields,
                        },
                    )
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(post, path = #publish_path, tags = [#name])]
            pub async fn publish_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(events):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::EventBus>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Path(id):
                    yeollin_plugin::vespera::axum::extract::Path<String>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .publish(&events, &current.sub, &id)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(post, path = #unpublish_path, tags = [#name])]
            pub async fn unpublish_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(events):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::EventBus>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Path(id):
                    yeollin_plugin::vespera::axum::extract::Path<String>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .unpublish(&events, &current.sub, &id)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }

            #[yeollin_plugin::vespera::route(delete, path = #item_path, tags = [#name])]
            pub async fn delete_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::Extension(events):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::EventBus>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::extract::Path(id):
                    yeollin_plugin::vespera::axum::extract::Path<String>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#delete_response>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                let deleted_id = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .delete(&events, &current.sub, &id)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(#delete_response {
                    success: true,
                    deleted_id,
                }))
            }

            #[yeollin_plugin::vespera::route(get, path = #published_path, tags = [#name])]
            pub async fn published_content(
                yeollin_plugin::vespera::axum::Extension(db):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::DatabaseConnection>,
                yeollin_plugin::vespera::axum::extract::Query(query):
                    yeollin_plugin::vespera::axum::extract::Query<#published_query>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#response>, yeollin_plugin::PluginError> {
                let result = yeollin_plugin::ContentRepository::<#fields>::new(db, #name)
                    .published(&query.slug)
                    .await?;
                Ok(yeollin_plugin::vespera::axum::Json(result.into()))
            }
        }
    })
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
    api_base: Option<LitStr>,
    settings: Option<Path>,
    collections: Vec<Expr>,
    subscribers: Vec<Expr>,
    public_api_routes: Vec<LitStr>,
    runtime_storage: bool,
    request_body_limit: Option<Expr>,
}

impl Parse for PluginDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name: Option<LitStr> = None;
        let mut author: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;
        let mut on_init: Option<Expr> = None;
        let mut frontend: Option<bool> = None;
        let mut api_base: Option<LitStr> = None;
        let mut settings: Option<Path> = None;
        let mut collections = vec![];
        let mut subscribers = vec![];
        let mut public_api_routes = vec![];
        let mut runtime_storage = false;
        let mut request_body_limit = None;

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
                "api_base" => {
                    api_base = Some(input.parse()?);
                }
                "settings" => {
                    settings = Some(input.parse()?);
                }
                "collections" => {
                    let content;
                    bracketed!(content in input);
                    collections = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?
                        .into_iter()
                        .collect();
                }
                "subscribers" => {
                    let content;
                    bracketed!(content in input);
                    subscribers = Punctuated::<Expr, Token![,]>::parse_terminated(&content)?
                        .into_iter()
                        .collect();
                }
                "public_api_routes" => {
                    let content;
                    bracketed!(content in input);
                    public_api_routes =
                        Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
                            .into_iter()
                            .collect();
                }
                "runtime_storage" => {
                    let val: LitBool = input.parse()?;
                    runtime_storage = val.value();
                }
                "request_body_limit" => {
                    request_body_limit = Some(input.parse()?);
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
            api_base,
            settings,
            collections,
            subscribers,
            public_api_routes,
            runtime_storage,
            request_body_limit,
        })
    }
}

/// Every plugin API lives under this segment. It is prepended by the framework
/// and must not be repeated in `api_base`.
const API_ROOT: &str = "/api";

/// Lower-case and hyphenate, since `-` is the conventional URL word separator.
fn to_kebab_case(value: &str) -> String {
    value.trim().to_lowercase().replace('_', "-")
}

/// Resolve the URL prefix a plugin's routes are mounted under.
///
/// Defaults to the plugin name so that the common case declares nothing, and
/// so that a plugin's API namespace matches the frontend namespace it already
/// gets from the same name.
fn resolve_api_prefix(def: &PluginDef) -> syn::Result<String> {
    let Some(base) = def.api_base.as_ref() else {
        return Ok(format!("{API_ROOT}/{}", to_kebab_case(&def.name.value())));
    };

    let raw = base.value();
    let trimmed = raw.trim().trim_matches('/');

    if trimmed.is_empty() {
        return Err(syn::Error::new(
            base.span(),
            "`api_base` must not be empty. Omit it to derive the base from `name`.",
        ));
    }

    // `/api` is structural, not something a plugin opts into, so accepting it
    // here would silently produce `/api/api/...`.
    let first = trimmed.split('/').next().unwrap_or_default();
    if first.eq_ignore_ascii_case("api") {
        return Err(syn::Error::new(
            base.span(),
            format!(
                "`api_base` must not start with `api`; every plugin API is already mounted under `{API_ROOT}`. \
                 Write `api_base: \"{}\"`.",
                trimmed.strip_prefix(first).unwrap_or("").trim_matches('/')
            ),
        ));
    }

    Ok(format!("{API_ROOT}/{}", to_kebab_case(trimmed)))
}

/// Resolve a public suffix below a plugin's API namespace.
///
/// Public access is path-based rather than method-based, so the namespace root
/// itself is forbidden: making `/api/media` public for GET would also expose a
/// POST mounted at the same path. Dynamic and query-bearing declarations are
/// likewise incompatible with exact matching.
fn resolve_public_api_route(api_prefix: &str, suffix: &LitStr) -> syn::Result<String> {
    let raw = suffix.value();
    if !raw.starts_with('/') || raw == "/" {
        return Err(syn::Error::new(
            suffix.span(),
            "a public API route must be a non-root path beginning with `/`",
        ));
    }
    if raw.ends_with('/')
        || raw.contains(['?', '#', '{', '}', '*'])
        || raw.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        return Err(syn::Error::new(
            suffix.span(),
            "a public API route must be a fixed path without a trailing slash, query, fragment, wildcard, or traversal segment",
        ));
    }
    Ok(format!("{api_prefix}{raw}"))
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

/// Compile the crate's `app/` routes while the macro expands.
///
/// A deployed binary no longer has the build machine's source tree, so routes —
/// and with them every `public` and `guest` access rule — are baked in here
/// rather than rediscovered from disk at startup. Invalid metadata fails the
/// build instead of the running server.
fn compile_embedded_routes(plugin: Option<&str>) -> String {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return "[]".to_string();
    };

    let app_dir = std::path::Path::new(&manifest_dir).join("app");
    if !app_dir.is_dir() {
        return "[]".to_string();
    }

    let source = match plugin {
        Some(name) => yeollin_core::RouteSource::plugin(name, &app_dir),
        None => yeollin_core::RouteSource::app(&app_dir),
    };

    match yeollin_core::compile_route_manifest(&[source]) {
        Ok(manifest) => serde_json::to_string(&manifest.routes)
            .unwrap_or_else(|error| panic!("could not serialize route manifest: {error}")),
        Err(diagnostics) => {
            let details: String = diagnostics
                .iter()
                .map(|diagnostic| format!("\n  {diagnostic}"))
                .collect();
            panic!("invalid route metadata:{details}")
        }
    }
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
        let embedded = compile_embedded_routes(Some(&name_str));
        quote! {
            .frontend(yeollin_plugin::FrontendAssets::compile(
                #name_lit,
                FRONTEND_PATH,
                #embedded,
            ))
            .frontend_path(FRONTEND_PATH)
        }
    } else {
        quote! {}
    };

    let api_prefix = match resolve_api_prefix(&def) {
        Ok(prefix) => prefix,
        Err(error) => return TokenStream::from(error.to_compile_error()),
    };

    let public_api_routes = match def
        .public_api_routes
        .iter()
        .map(|suffix| resolve_public_api_route(&api_prefix, suffix))
        .collect::<syn::Result<Vec<_>>>()
    {
        Ok(routes) => routes,
        Err(error) => return TokenStream::from(error.to_compile_error()),
    };
    let mut unique_public_routes = std::collections::HashSet::new();
    for route in &public_api_routes {
        if !unique_public_routes.insert(route) {
            return TokenStream::from(
                syn::Error::new(
                    name_lit.span(),
                    format!("duplicate public API route `{route}`"),
                )
                .to_compile_error(),
            );
        }
    }
    let public_api_setters = public_api_routes.iter().map(|route| {
        let route = LitStr::new(route, name_lit.span());
        quote! { .public_api_route(#route) }
    });
    let runtime_storage_setter = def
        .runtime_storage
        .then(|| quote! { .requires_runtime_storage() });
    let request_body_limit_setter = def.request_body_limit.as_ref().map(|limit| {
        quote! { .request_body_limit(#limit) }
    });

    let settings_tokens = def.settings.as_ref().map(|settings_type| {
        quote! {
            pub async fn __yeollin_get_plugin_settings(
                yeollin_plugin::vespera::axum::Extension(settings):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::SettingsStore>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#settings_type>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                Ok(yeollin_plugin::vespera::axum::Json(
                    settings.get::<#settings_type>(#name_lit).await?
                ))
            }

            pub async fn __yeollin_put_plugin_settings(
                yeollin_plugin::vespera::axum::Extension(settings):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::SettingsStore>,
                yeollin_plugin::vespera::axum::Extension(current):
                    yeollin_plugin::vespera::axum::Extension<yeollin_plugin::CurrentUser>,
                yeollin_plugin::vespera::axum::Json(value):
                    yeollin_plugin::vespera::axum::Json<#settings_type>,
            ) -> Result<yeollin_plugin::vespera::axum::Json<#settings_type>, yeollin_plugin::PluginError> {
                yeollin_plugin::Authorize::require_role(&current, "admin")?;
                Ok(yeollin_plugin::vespera::axum::Json(
                    settings.set::<#settings_type>(#name_lit, value).await?
                ))
            }
        }
    });

    let metadata_router = if def.settings.is_some() {
        let api_path = LitStr::new(&format!("{api_prefix}/settings"), name_lit.span());
        quote! {
            yeollin_plugin::vespera::axum::Router::new().route(
                #api_path,
                yeollin_plugin::vespera::axum::routing::get(__yeollin_get_plugin_settings)
                    .put(__yeollin_put_plugin_settings),
            )
        }
    } else {
        quote! { yeollin_plugin::vespera::axum::Router::new() }
    };

    let settings_setter = def.settings.as_ref().map(|settings_type| {
        let api_path = LitStr::new(&format!("{api_prefix}/settings"), name_lit.span());
        let page_path = LitStr::new(
            &format!("/{}/settings", to_kebab_case(&name_str)),
            name_lit.span(),
        );
        let custom_page = std::env::var("CARGO_MANIFEST_DIR")
            .map(|dir| {
                std::path::Path::new(&dir)
                    .join("app")
                    .join("settings")
                    .join("page.tsx")
                    .is_file()
            })
            .unwrap_or(false);

        quote! {
            .settings(yeollin_plugin::SettingsRegistration::new::<#settings_type>(
                #name_lit,
                yeollin_plugin::serde_json::to_value(
                    yeollin_plugin::vespera::schema!(#settings_type)
                ).expect("Vespera settings schema must serialize"),
                #api_path,
                #page_path,
                #custom_page,
            ))
        }
    });
    let subscriber_setters = def.subscribers.iter().map(|subscriber| {
        quote! { .subscriber(#subscriber) }
    });
    let api_prefix_lit = LitStr::new(&api_prefix, name_lit.span());
    let collection_setters = def.collections.iter().map(|collection| {
        quote! {
            .content_collection((#collection).for_plugin(#name_lit, #api_prefix_lit))
        }
    });

    let expanded = quote! {
        #settings_tokens

        // Auto-generate export_app with PascalCase name derived from plugin name.
        // The prefix mounts every route under the plugin's API namespace, so a
        // handler's URL comes from the declaration rather than its file location.
        yeollin_plugin::vespera::export_app!(#export_ident, prefix = #api_prefix);

        #frontend_path_const

        #on_init_fn

        /// Plugin metadata entry point
        pub fn metadata() -> yeollin_plugin::PluginMetadata {
            yeollin_plugin::PluginMetadata::builder(#name_lit, env!("CARGO_PKG_VERSION"))
                #author_setter
                #description_setter
                .license(env!("CARGO_PKG_LICENSE"))
                .router(#metadata_router)
                #on_init_setter
                #frontend_setters
                #settings_setter
                #(#collection_setters)*
                #(#subscriber_setters)*
                #(#public_api_setters)*
                #runtime_storage_setter
                #request_body_limit_setter
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

    let app_routes = compile_embedded_routes(None);

    let expanded = quote! {
        yeollin::app()
            // Registered unconditionally: when the app has no `app/` directory the
            // compiler yields no routes, which is the same fail-closed outcome as
            // not registering it.
            .app_frontend(concat!(env!("CARGO_MANIFEST_DIR"), "/app"), #app_routes)
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

#[cfg(test)]
mod api_base_tests {
    use super::{
        resolve_api_prefix, resolve_public_api_route, validate_collection_name,
        ContentCollectionDef, PluginDef,
    };
    use syn::LitStr;

    fn prefix_of(declaration: &str) -> String {
        let def: PluginDef = syn::parse_str(declaration).expect("declaration must parse");
        resolve_api_prefix(&def).expect("prefix must resolve")
    }

    fn error_of(declaration: &str) -> String {
        let def: PluginDef = syn::parse_str(declaration).expect("declaration must parse");
        resolve_api_prefix(&def)
            .expect_err("prefix must be rejected")
            .to_string()
    }

    #[test]
    fn defaults_to_the_plugin_name() {
        assert_eq!(prefix_of(r#"name: "media-library""#), "/api/media-library");
    }

    #[test]
    fn underscores_become_hyphens() {
        assert_eq!(prefix_of(r#"name: "media_library""#), "/api/media-library");
        assert_eq!(
            prefix_of(r#"name: "x", api_base: "media_library""#),
            "/api/media-library"
        );
    }

    #[test]
    fn api_base_overrides_the_name() {
        assert_eq!(
            prefix_of(r#"name: "auth-users", api_base: "auth""#),
            "/api/auth"
        );
    }

    #[test]
    fn surrounding_slashes_are_tolerated() {
        assert_eq!(prefix_of(r#"name: "x", api_base: "/auth/""#), "/api/auth");
    }

    #[test]
    fn nested_bases_are_kept() {
        assert_eq!(
            prefix_of(r#"name: "x", api_base: "v1/reports""#),
            "/api/v1/reports"
        );
    }

    #[test]
    fn a_leading_api_segment_is_rejected() {
        // Accepting it would silently produce `/api/api/...`.
        for declaration in [
            r#"name: "x", api_base: "api/auth""#,
            r#"name: "x", api_base: "/api/auth""#,
            r#"name: "x", api_base: "API/auth""#,
        ] {
            assert!(
                error_of(declaration).contains("must not start with `api`"),
                "expected rejection for {declaration}"
            );
        }
    }

    #[test]
    fn an_empty_base_is_rejected() {
        assert!(error_of(r#"name: "x", api_base: "/""#).contains("must not be empty"));
    }

    #[test]
    fn accepts_a_settings_type() {
        let def: PluginDef = syn::parse_str(r#"name: "x", settings: crate::Settings"#).unwrap();

        assert_eq!(
            def.settings.unwrap().segments.last().unwrap().ident,
            "Settings"
        );
    }

    #[test]
    fn accepts_subscriber_registration_expressions() {
        let def: PluginDef =
            syn::parse_str(r#"name: "x", subscribers: [crate::first(), crate::second()]"#).unwrap();

        assert_eq!(def.subscribers.len(), 2);
    }

    #[test]
    fn accepts_content_collection_registrations() {
        let def: PluginDef =
            syn::parse_str(r#"name: "content", collections: [pages::registration()]"#).unwrap();

        assert_eq!(def.collections.len(), 1);
    }

    #[test]
    fn parses_a_typed_content_collection() {
        let def: ContentCollectionDef = syn::parse_str(
            r#"module: pages, name: "pages", label: "Pages", fields: crate::PageFields, order: 30"#,
        )
        .unwrap();

        assert_eq!(def.module, "pages");
        assert_eq!(def.name.value(), "pages");
        assert_eq!(def.label.value(), "Pages");
        assert_eq!(def.order.base10_parse::<i32>().unwrap(), 30);
    }

    #[test]
    fn rejects_non_canonical_content_collection_names() {
        for name in ["", "Pages", "blog_posts", "blog/pages", "-pages"] {
            assert!(validate_collection_name(&LitStr::new(
                name,
                proc_macro2::Span::call_site()
            ))
            .is_err());
        }
    }

    #[test]
    fn resolves_fixed_public_api_routes_below_the_namespace() {
        let def: PluginDef =
            syn::parse_str(r#"name: "media", public_api_routes: ["/file", "/thumbnail"]"#).unwrap();
        let prefix = resolve_api_prefix(&def).unwrap();

        assert_eq!(
            def.public_api_routes
                .iter()
                .map(|route| resolve_public_api_route(&prefix, route).unwrap())
                .collect::<Vec<_>>(),
            ["/api/media/file", "/api/media/thumbnail"]
        );
    }

    #[test]
    fn rejects_ambiguous_public_api_routes() {
        for suffix in ["/", "file", "/file/", "/{id}", "/../file", "/file?x=1"] {
            let literal = LitStr::new(suffix, proc_macro2::Span::call_site());
            assert!(resolve_public_api_route("/api/media", &literal).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::to_pascal_case;

    /// `yeollin_plugin!` derives the export identifier from the plugin's *name
    /// string* (hyphenated), while `yeollin_app!` derives the same identifier
    /// from the plugin's *module path* (underscored). If those two derivations
    /// ever disagree the generated app fails to link against the plugin.
    #[test]
    fn hyphenated_names_and_underscored_idents_agree() {
        for (name, module) in [
            ("example-plugin", "example_plugin"),
            ("example-memo-plugin", "example_memo_plugin"),
            ("auth", "auth"),
            ("a-b-c-d", "a_b_c_d"),
        ] {
            assert_eq!(
                to_pascal_case(name),
                to_pascal_case(module),
                "`{name}` and `{module}` must produce one export identifier"
            );
        }
    }

    #[test]
    fn produces_expected_export_identifiers() {
        assert_eq!(to_pascal_case("example-plugin"), "ExamplePlugin");
        assert_eq!(to_pascal_case("example-memo-plugin"), "ExampleMemoPlugin");
        assert_eq!(to_pascal_case("auth"), "Auth");
    }

    #[test]
    fn collapses_repeated_and_trailing_separators() {
        assert_eq!(to_pascal_case("a--b"), "AB");
        assert_eq!(to_pascal_case("-leading"), "Leading");
        assert_eq!(to_pascal_case("trailing-"), "Trailing");
        assert_eq!(to_pascal_case("mixed-_case"), "MixedCase");
    }

    #[test]
    fn preserves_digits_and_existing_capitals() {
        assert_eq!(to_pascal_case("plugin-v2"), "PluginV2");
        assert_eq!(to_pascal_case("s3-storage"), "S3Storage");
    }

    #[test]
    fn empty_input_yields_empty_identifier() {
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_pascal_case("---"), "");
    }
}
