# CRATES - RUST WORKSPACE

## OVERVIEW

Four crates: core (types), plugin (interface), app (runtime), cli (tooling).

## STRUCTURE

```
crates/
├── core/      # MenuConfig, shared types
├── plugin/    # PluginMetadata, yeollin_plugin! macro
├── app/       # YeollinApp, server, static/dev proxy
└── cli/       # yeollin-cli commands (dev, build, prebuild)
```

## WHERE TO LOOK

| Task | File | Notes |
|------|------|-------|
| Add plugin field | `plugin/src/metadata.rs` | Update struct + builder |
| Add plugin macro field | `plugin/src/macros.rs` | Uses CARGO_PKG_* env vars |
| Modify app builder | `app/src/app.rs` | YeollinAppBuilder methods |
| Add CLI command | `cli/src/commands/` | New module + update mod.rs |
| Plugin export logic | `cli/src/commands/prebuild.rs` | PluginInfo struct, detect_crate_dir() |
| Crate detection | `cli/src/commands/prebuild.rs` | `detect_crate_dir()` for flat/legacy support |

## KEY TYPES

| Type | Crate | Purpose |
|------|-------|---------|
| `PluginMetadata` | plugin | Plugin definition (routes + frontend) |
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
