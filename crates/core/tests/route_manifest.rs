use std::fs;
use std::path::Path;

use tempfile::TempDir;
use yeollin_core::{
    compile_route_manifest, RouteAccess, RouteSource, ROUTE_MANIFEST_SCHEMA_VERSION,
};

fn page(root: &Path, relative: &str) {
    let dir = root.join(relative);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("page.tsx"), "export default function Page() {}").unwrap();
}

fn meta(root: &Path, relative: &str, json: &str) {
    let dir = root.join(relative);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("route.meta.json"), json).unwrap();
}

/// App tree with a route group, a public page, a guest page, a nested page and a
/// dynamic segment; plus a plugin tree with an ordered menu entry.
fn fixture() -> (TempDir, Vec<RouteSource>) {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    let plugin = tmp.path().join("plugin-app");

    page(&app, "(dashboard)");
    meta(&app, "(dashboard)", r#"{"label":"Dashboard","order":0}"#);

    page(&app, "(public)/test");
    meta(&app, "(public)/test", r#"{"access":"public","menu":false}"#);

    page(&app, "(guest)/signin");
    meta(&app, "(guest)/signin", r#"{"access":"guest","menu":false}"#);

    page(&app, "(dashboard)/settings");
    page(&app, "(dashboard)/settings/plugins");
    page(&app, "(dashboard)/reports/[id]");

    page(&plugin, "(memo)");
    meta(&plugin, "(memo)", r#"{"label":"Memo","icon":"note","order":10}"#);
    page(&plugin, "(memo)/archive");

    let sources = vec![
        RouteSource::app(&app),
        RouteSource::plugin("memo-plugin", &plugin),
    ];
    (tmp, sources)
}

#[test]
fn route_manifest_fixture_is_deterministic() {
    let (_tmp, sources) = fixture();

    let manifest = compile_route_manifest(&sources).expect("fixture must compile");

    assert_eq!(manifest.schema_version, ROUTE_MANIFEST_SCHEMA_VERSION);

    let rendered: Vec<String> = manifest
        .routes
        .iter()
        .map(|route| {
            format!(
                "{} plugin={:?} label={} order={} access={:?} menu={}",
                route.path, route.plugin, route.label, route.order, route.access, route.menu
            )
        })
        .collect();

    // Sorted by (order, plugin, path): application routes precede plugin routes
    // at equal order, so menu placement never depends on directory scan order.
    assert_eq!(
        rendered,
        vec![
            "/ plugin=None label=Dashboard order=0 access=Authenticated menu=true",
            "/memo-plugin plugin=Some(\"memo-plugin\") label=Memo order=10 access=Authenticated menu=true",
            "/reports/[id] plugin=None label=[id] order=50 access=Authenticated menu=false",
            "/settings plugin=None label=settings order=50 access=Authenticated menu=true",
            "/settings/plugins plugin=None label=plugins order=50 access=Authenticated menu=true",
            "/signin plugin=None label=signin order=50 access=Guest menu=false",
            "/test plugin=None label=test order=50 access=Public menu=false",
            "/memo-plugin/archive plugin=Some(\"memo-plugin\") label=archive order=50 access=Authenticated menu=true",
        ]
    );

    // Recompiling the same tree must not reorder anything.
    let again = compile_route_manifest(&sources).expect("fixture must compile");
    assert_eq!(manifest, again);
}

#[test]
fn route_groups_never_grant_access() {
    let (_tmp, sources) = fixture();
    let manifest = compile_route_manifest(&sources).unwrap();

    // `(public)`/`(guest)` directory names are organisational. Only the sidecars
    // placed inside them may widen access.
    assert_eq!(manifest.paths_with_access(RouteAccess::Public), vec!["/test"]);
    assert_eq!(
        manifest.paths_with_access(RouteAccess::Guest),
        vec!["/signin"]
    );

    let authenticated = manifest.paths_with_access(RouteAccess::Authenticated);
    for expected in ["/", "/settings", "/settings/plugins", "/memo-plugin"] {
        assert!(
            authenticated.contains(&expected.to_string()),
            "{expected} must default to authenticated"
        );
    }
}

#[test]
fn undeclared_routes_default_to_authenticated() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(public)/looks-public");

    let manifest = compile_route_manifest(&[RouteSource::app(&app)]).unwrap();

    assert_eq!(manifest.routes.len(), 1);
    assert_eq!(manifest.routes[0].path, "/looks-public");
    assert_eq!(manifest.routes[0].access, RouteAccess::Authenticated);
}

#[test]
fn unknown_metadata_fields_fail_the_build() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/typo");
    meta(&app, "(main)/typo", r#"{"acess":"public"}"#);

    let diagnostics = compile_route_manifest(&[RouteSource::app(&app)])
        .expect_err("a misspelled key must not silently default");

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains("not valid route metadata"),
        "unexpected diagnostic: {}",
        diagnostics[0]
    );
}

#[test]
fn invalid_access_values_fail_the_build() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/bad");
    meta(&app, "(main)/bad", r#"{"access":"everyone"}"#);

    let diagnostics = compile_route_manifest(&[RouteSource::app(&app)]).expect_err("must reject");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn malformed_metadata_fails_the_build() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/broken");
    meta(&app, "(main)/broken", "{ not json");

    let diagnostics = compile_route_manifest(&[RouteSource::app(&app)]).expect_err("must reject");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn colliding_routes_fail_the_build() {
    let tmp = TempDir::new().unwrap();
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    page(&first, "(a)/reports");
    page(&second, "(b)/reports");

    let diagnostics =
        compile_route_manifest(&[RouteSource::app(&first), RouteSource::app(&second)])
            .expect_err("duplicate paths must not silently overwrite");

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains("already defined"),
        "unexpected diagnostic: {}",
        diagnostics[0]
    );
}

#[test]
fn dynamic_routes_cannot_be_forced_into_the_menu() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/posts/[slug]");
    meta(&app, "(main)/posts/[slug]", r#"{"menu":true}"#);

    let diagnostics = compile_route_manifest(&[RouteSource::app(&app)]).expect_err("must reject");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0].message.contains("dynamic segment"),
        "unexpected diagnostic: {}",
        diagnostics[0]
    );
}

#[test]
fn every_broken_sidecar_is_reported_at_once() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/one");
    meta(&app, "(main)/one", r#"{"acess":"public"}"#);
    page(&app, "(main)/two");
    meta(&app, "(main)/two", "{ not json");

    let diagnostics = compile_route_manifest(&[RouteSource::app(&app)]).expect_err("must reject");
    assert_eq!(diagnostics.len(), 2, "both sidecars must be reported");
}

#[test]
fn directories_without_a_page_are_not_routes() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    fs::create_dir_all(app.join("(main)/components")).unwrap();
    fs::write(app.join("(main)/components/Button.tsx"), "export {}").unwrap();
    page(&app, "(main)/real");

    let manifest = compile_route_manifest(&[RouteSource::app(&app)]).unwrap();

    let paths: Vec<&str> = manifest.routes.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["/real"]);
}

#[test]
fn private_and_hidden_directories_are_skipped() {
    let tmp = TempDir::new().unwrap();
    let app = tmp.path().join("app");
    page(&app, "(main)/_internal");
    page(&app, "(main)/.cache");
    page(&app, "(main)/visible");

    let manifest = compile_route_manifest(&[RouteSource::app(&app)]).unwrap();

    let paths: Vec<&str> = manifest.routes.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, vec!["/visible"]);
}
