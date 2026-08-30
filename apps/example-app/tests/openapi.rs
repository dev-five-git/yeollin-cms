//! Guards the generated OpenAPI document.
//!
//! Vespera reads handler return types syntactically. A change that hides the
//! success type behind a type alias still compiles, still serves correct JSON,
//! and still passes every other test — it only degrades the published schema to
//! `{"type":"object"}`, which then produces wrong types in generated clients.
//! Nothing else in the suite notices, so these assertions do.

use std::path::PathBuf;

use serde_json::Value;

fn spec() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("openapi.json");

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "could not read the generated spec at {}: {error}",
            path.display()
        )
    });

    serde_json::from_str(&raw).expect("generated spec must be valid JSON")
}

fn collect_refs(node: &Value, found: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "$ref" {
                    if let Some(reference) = value.as_str() {
                        found.push(reference.to_string());
                    }
                } else {
                    collect_refs(value, found);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect_refs(item, found)),
        _ => {}
    }
}

#[test]
fn documented_responses_reference_named_schemas() {
    let spec = spec();

    // Each of these handlers returns a named type. A bare object here means the
    // return type stopped being resolvable, not that the endpoint changed.
    for (path, method) in [
        ("/api/example-memo-plugin", "get"),
        ("/api/example-memo-plugin/{id}", "get"),
        ("/api/auth/login", "post"),
        ("/api/auth/me", "get"),
    ] {
        let schema = &spec["paths"][path][method]["responses"]["200"]["content"]
            ["application/json"]["schema"];

        assert!(
            schema.get("$ref").is_some(),
            "{method} {path} lost its schema reference and is documented as {schema}. \
             A `#[vespera::route]` handler must spell out `Result<Json<T>, PluginError>`; \
             a type alias such as `PluginResult<Json<T>>` is not resolved."
        );
    }
}

#[test]
fn every_reference_resolves_to_a_component() {
    let spec = spec();

    let mut refs = Vec::new();
    collect_refs(&spec["paths"], &mut refs);
    assert!(!refs.is_empty(), "spec should reference some components");

    for reference in refs {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .unwrap_or_else(|| panic!("unexpected reference form: {reference}"));

        assert!(
            spec["components"]["schemas"].get(name).is_some(),
            "{reference} points at a component that is not in the document"
        );
    }
}

#[test]
fn every_plugin_route_lives_under_its_declared_namespace() {
    let spec = spec();

    let paths: Vec<&str> = spec["paths"]
        .as_object()
        .expect("paths must be an object")
        .keys()
        .map(String::as_str)
        .collect();

    // A plugin's public URL comes from its `name` (or `api_base`), never from
    // where its handler files happen to sit. Moving a handler must not move
    // the endpoint, so the full set is pinned here.
    let mut actual: Vec<&str> = paths.clone();
    actual.sort_unstable();

    assert_eq!(
        actual,
        vec![
            "/api/audit-log",
            "/api/auth/login",
            "/api/auth/logout",
            "/api/auth/me",
            "/api/auth/password",
            "/api/auth/refresh",
            "/api/auth/users",
            "/api/auth/users/{id}",
            "/api/auth/users/{id}/password",
            "/api/dashboard/stats",
            "/api/example-memo-plugin",
            "/api/example-memo-plugin/{id}",
            "/api/example-plugin/items/",
            "/api/example-plugin/items/{id}",
            "/api/media",
            "/api/media/file",
            "/api/media/{id}",
            "/api/search",
            "/api/webhooks",
            "/api/webhooks/deliveries",
            "/api/webhooks/deliveries/{id}/retry",
            "/api/webhooks/{id}",
        ]
    );

    for path in paths {
        assert!(
            path.starts_with("/api/"),
            "{path} is a backend route and must live under /api"
        );
        assert!(
            !path.contains('_'),
            "{path} should use hyphens; underscores are not the URL convention"
        );
    }
}

#[test]
fn schema_names_are_unique_across_plugins() {
    let spec = spec();

    let schemas = spec["components"]["schemas"]
        .as_object()
        .expect("components.schemas must be an object");

    // Two plugins each defining their own `ErrorResponse` used to collapse into a
    // single component, silently publishing one plugin's shape for both.
    assert!(
        !schemas.contains_key("ErrorResponse"),
        "plugins must share the framework error body instead of each declaring `ErrorResponse`"
    );
}
