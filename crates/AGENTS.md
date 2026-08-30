# CRATES - RUST WORKSPACE

## OVERVIEW

Six crates: core (types + route manifest + export contract), auth (JWT/Argon2/middleware),
plugin (interface), plugin-macros (proc macros), app (runtime), cli (tooling).

## STRUCTURE

```
crates/
├── core/           # MenuConfig, RouteManifest, ExportEnvelope, shared types
├── auth/           # AuthConfig, JWT, Argon2, auth_middleware
├── plugin/         # PluginMetadata, FrontendAssets
├── plugin-macros/  # yeollin_plugin!, yeollin_app!
├── app/            # YeollinApp, server, static/dev proxy
└── cli/            # yeollin-cli commands (init, prebuild, dev, build)
```

## WHERE TO LOOK

| Task | File | Notes |
|------|------|-------|
| Add plugin field | `plugin/src/metadata.rs` | Update struct + builder |
| Add plugin macro field | `plugin-macros/src/lib.rs` | Uses CARGO_PKG_* env vars |
| Modify app builder | `app/src/app.rs` | YeollinAppBuilder methods |
| Add CLI command | `cli/src/commands/` | New module + update mod.rs |
| Route metadata / menus | `core/src/route.rs` | `compile_route_manifest()`, `build_menu()` |
| Event bus / outbox | `core/src/events.rs` | Typed emit, Inline dispatch, Deferred drainer |
| App ↔ CLI metadata contract | `core/src/export.rs` | `ExportEnvelope`, `EXPORT_ENV_VAR` |
| Read metadata from a binary | `cli/src/commands/prebuild.rs` | `export_metadata()`, `parse_export_envelope()` |
| Crate detection | `cli/src/commands/prebuild.rs` | `detect_crate_dir()` for flat/legacy support |

## ROUTE METADATA (SECURITY-RELEVANT)

Page routes are discovered from the App Router tree, but **access rules are never
inferred from directory names**. A `route.meta.json` sidecar next to `page.tsx`
declares them:

```json
{ "label": "Items", "icon": "box", "order": 10, "access": "public", "menu": false }
```

- `access` is one of `authenticated` (default), `public`, `guest`.
- Route groups such as `(public)` / `(guest)` organise files and **grant nothing**.
- Unknown fields, invalid values, duplicate paths, and `menu: true` on a dynamic
  route all fail the build. There is no silent fallback.
- `menu` affects navigation only, never authorization.

`YeollinAppBuilder::app_frontend()` registers the host app's own `app/` directory;
`yeollin_app!` wires it automatically.

## METADATA EXPORT PROTOCOL

`prebuild` runs the built binary once with `YEOLLIN_EXPORT=1`. The binary must:

1. Emit exactly one `ExportEnvelope` JSON document on **stdout** and nothing else.
2. Send all logs to **stderr** (`fmt::layer().with_writer(std::io::stderr)`).
3. Exit before connecting to a database, running `on_init`, or requiring secrets.

Use `with_database_url()` rather than `with_database()` so the connection is
opened lazily and export stays side-effect free.

## KEY TYPES

| Type | Crate | Purpose |
|------|-------|---------|
| `PluginMetadata` | plugin | Plugin definition (routes + frontend) |
| `EventBus` / `EventTransaction` | core | Transactional event outbox and delivery |
| `YeollinAppBuilder` | app | Fluent API for CMS setup |
| `PluginInfo` | cli | Serializable plugin data for JSON |
| `PluginFrontend` | cli | Frontend paths for prebuild |

## CRATE DETECTION (CLI)

The CLI supports both new flat structure and legacy `api/` subdirectory:

```rust
// crates/cli/src/commands/prebuild.rs
fn detect_crate_dir(base: &Path) -> Option<PathBuf> {
    // Check flat structure first (Cargo.toml at root)
    if base.join("Cargo.toml").exists() {
        return Some(base.to_path_buf());
    }
    // Fallback to legacy api/ subdirectory
    let api_dir = base.join("api");
    if api_dir.join("Cargo.toml").exists() {
        return Some(api_dir);
    }
    None
}
```

## CONVENTIONS

- Workspace dependencies in root `Cargo.toml`
- Re-exports via `pub use` in lib.rs
- Vespera for OpenAPI route generation
- Extension layer for shared state (SharedMenus, SharedPlugins)
- `EventBus` is an Extension; event-producing writes use `EventTransaction::connection()`
- Inline subscriber errors abort the action; Deferred delivery wakes after commit and also polls
- **Plugin frontend path**: `concat!(env!("CARGO_MANIFEST_DIR"), "/app")` (flat structure)

## CLI COMMANDS

| Command | Purpose |
|---------|---------|
| `prebuild` | Extract template, link plugins, generate manifests |
| `dev` | Build → prebuild → run vinext + API (single port) |
| `build` | Full production build (frontend + backend) |

## APP/PLUGIN STRUCTURE

Both apps (`apps/`) and plugins (`plugins/`) now use flat structure:

```
my-plugin/           # or my-app/
├── Cargo.toml       # At root (NOT in api/)
├── src/
│   └── lib.rs       # or main.rs for apps
├── app/             # Frontend pages
└── package.json     # TypeScript DX
```

## ANTI-PATTERNS

- **NO** blocking calls in async handlers
- **NO** manual JSON serialization (use serde derive)
- **NO** hardcoded paths (use CARGO_MANIFEST_DIR)
- **NO** `api/` subdirectory in new plugins (use flat structure)
