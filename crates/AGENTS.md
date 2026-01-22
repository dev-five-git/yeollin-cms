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
| Plugin export logic | `cli/src/commands/prebuild.rs` | PluginInfo struct |

## KEY TYPES

| Type | Crate | Purpose |
|------|-------|---------|
| `PluginMetadata` | plugin | Plugin definition (routes + frontend) |
| `YeollinAppBuilder` | app | Fluent API for CMS setup |
| `PluginInfo` | cli | Serializable plugin data for JSON |
| `PluginFrontend` | cli | Frontend paths for prebuild |

## CONVENTIONS

- Workspace dependencies in root `Cargo.toml`
- Re-exports via `pub use` in lib.rs
- Vespera for OpenAPI route generation
- Extension layer for shared state (SharedMenus, SharedPlugins)

## CLI COMMANDS

| Command | Purpose |
|---------|---------|
| `prebuild` | Extract template, link plugins, generate manifests |
| `dev` | Build → prebuild → run Next.js + API (single port) |
| `build` | Full production build (frontend + backend) |

## ANTI-PATTERNS

- **NO** blocking calls in async handlers
- **NO** manual JSON serialization (use serde derive)
- **NO** hardcoded paths (use CARGO_MANIFEST_DIR)
