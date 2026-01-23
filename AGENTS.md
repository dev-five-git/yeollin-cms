# YEOLLIN CMS - PROJECT KNOWLEDGE BASE

**Generated:** 2026-01-23
**Commit:** 5bcfd78
**Branch:** main

## OVERVIEW

Tauri-inspired CMS framework: Rust/Axum backend + Next.js frontend. Plugins bundle API routes + frontend UI in single crates.

## STRUCTURE

```
yeollin-cms/
├── crates/           # Rust workspace (core, plugin, app, cli)
├── packages/         # Node workspace (app = Next.js frontend template)
├── plugins/          # Plugin crates (example-plugin)
├── apps/             # Standalone app crates (example-app)
└── .yeollin/         # Generated at prebuild (gitignored)
```

## APP/PLUGIN STRUCTURE (NEW)

Both apps and plugins now use the same flat structure with Cargo.toml at root:

```
my-plugin/            # or my-app/
├── Cargo.toml        # Rust crate manifest at root
├── src/              # Rust source code
│   ├── lib.rs        # Plugin: yeollin_plugin! macro
│   └── main.rs       # App: entry point
├── app/              # Frontend (Next.js pages)
│   └── (group)/      # Route group
├── package.json      # Node deps for TypeScript DX
└── tsconfig.json     # TypeScript config
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Plugin interface | `crates/plugin/src/metadata.rs` | PluginMetadata struct, builder pattern |
| App builder | `crates/app/src/app.rs` | YeollinAppBuilder, plugin registration |
| CLI commands | `crates/cli/src/commands/` | dev, build, prebuild, init |
| Frontend template | `packages/app/` | Extracted to `.yeollin/app/` at prebuild |
| Plugin example | `plugins/example-plugin/` | Library plugin (lib.rs only) |
| Standalone app | `apps/example-app/` | Complete CMS with main.rs |

## TECH STACK

| Layer | Stack |
|-------|-------|
| Backend | Rust, Axum 0.8, Vespera (OpenAPI), sea-orm |
| Frontend | Next.js 16, React 19, @devup-ui/react, @devup-api/fetch |
| Build | Cargo workspace, bun workspaces |
| Dev | yeollin-cli (prebuild, dev, build) |

## KEY PATTERNS

### Plugin Registration
```rust
// plugins/my-plugin/src/lib.rs
yeollin_plugin::yeollin_plugin! {
    name: "my-plugin",
    author: "...",
    description: "...",
}
```

### Dev Mode (Single Port)
- Port 3001: Rust API + dev proxy
- Port 3000: Internal Next.js dev server (proxied)
- WebSocket proxy for HMR at `/_next/webpack-hmr`

### Build Flow
1. `cargo build` → binary with YEOLLIN_EXPORT_PLUGINS support
2. `yeollin prebuild` → extract template, link plugins, generate menus.json
3. `bun run build` → Next.js SSG to `.yeollin/app/out/`
4. `cargo build --release` → embeds static files via include_dir!

## CONVENTIONS

- **Typography tokens**: `heading`, `subheading`, `body`, `label` (NOT title/caption)
- **Plugin frontend**: `plugins/<name>/app/` with `(group)/` route groups
- **Routes**: Vespera macros `#[vespera::route(get, path = "...", tags = ["..."])]`
- **State**: Extension layer for SharedMenus, SharedPlugins

## ANTI-PATTERNS

- **NO** `typography="title"` or `typography="caption"` (use subheading/label)
- **NO** fetch() in SSG pages (use file reads for build-time data)
- **NO** direct edits to `.yeollin/` (regenerated at prebuild)
- **NO** symlinks in dev mode (use copy_mode=true for Turbopack)

## COMMANDS

```bash
# Development (from app dir)
cd apps/example-app
cargo run -p yeollin-cli -- dev     # Single port dev server

# Build
cargo run -p yeollin-cli -- build   # Full production build

# Check
cargo check --workspace             # Rust
bun tsc --noEmit                    # TypeScript (in packages/app)
```

## ENV VARS

| Var | Purpose |
|-----|---------|
| `PORT` | API server port (default: 3001) |
| `YEOLLIN_DEV_PROXY` | Enable dev proxy to Next.js port |
| `YEOLLIN_EXPORT_PLUGINS` | Export plugin JSON and exit |

## NOTES

- `packages/app/` is the TEMPLATE, not the running app
- Actual frontend runs from `.yeollin/app/` after prebuild
- Plugin frontend paths: `concat!(env!("CARGO_MANIFEST_DIR"), "/app")`
- menus.json and plugins.json generated at prebuild time
