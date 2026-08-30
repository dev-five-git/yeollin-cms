# YEOLLIN CMS - PROJECT KNOWLEDGE BASE

## OVERVIEW

Tauri-inspired CMS framework: Rust/Axum backend + vinext/Vite frontend. Plugins bundle API routes + frontend UI in single crates.

## STRUCTURE

```
yeollin-cms/
├── crates/           # Rust workspace (core, plugin, app, cli)
├── packages/         # Node workspace (app = vinext frontend template)
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
├── app/              # Frontend (vinext App Router pages)
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
| Frontend | vinext, Vite 8, React 19, @devup-ui/react, @devup-api/fetch |
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
- Port 3000: Internal vinext dev server (proxied)
- WebSocket proxy for Vite HMR at `/__vite_hmr`

### Build Flow
1. `cargo build` → binary that answers `YEOLLIN_EXPORT=1`
2. `yeollin prebuild` → extract template, link plugins, generate menus.json / plugins.json / route-manifest.json
3. `bun run --bun build` → vinext emits `.yeollin/app/dist/client/`
4. CLI copies the static client output to `.yeollin/app/out/`
5. `cargo build --release` → embeds static files via include_dir!

## CONVENTIONS

- **Typography tokens**: `heading`, `subheading`, `body`, `label` (NOT title/caption)
- **Plugin frontend**: `plugins/<name>/app/` with `(group)/` route groups
- **Route metadata**: `route.meta.json` next to `page.tsx`. `access` is `authenticated`
  (default) / `public` / `guest`. Directory names grant nothing
- **Routes**: Vespera macros `#[vespera::route(get, path = "...", tags = ["..."])]`
- **State**: Extension layer for SharedMenus, SharedPlugins
- **Logs go to stderr**: stdout is reserved for the `YEOLLIN_EXPORT` envelope

## ANTI-PATTERNS

- **NO** `typography="title"` or `typography="caption"` (use subheading/label)
- **NO** fetch() in SSG pages (use file reads for build-time data)
- **NO** direct edits to `.yeollin/` (regenerated at prebuild)
- **NO** symlinks in dev mode (use proxy or copy mode)
- **NO** deriving access from `(public)` / `(guest)` directory names — declare it in `route.meta.json`
- **NO** prefix matching for public/guest routes — matching is whole-path exact
- **NO** work before the export branch in `YeollinApp::run` (no DB, no secrets)

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
| `JWT_SECRET` | **Required to serve.** Min 32 bytes; startup fails otherwise. `yeollin dev` injects an ephemeral one |
| `YEOLLIN_DEV_PROXY` | Enable dev proxy to vinext port. Also gates the dev-asset auth exemption |
| `YEOLLIN_EXPORT` | Print one `ExportEnvelope` JSON on stdout and exit (used by prebuild) |

## NOTES

- `packages/app/` is the TEMPLATE, not the running app
- Actual frontend runs from `.yeollin/app/` after prebuild
- Plugin frontend paths: `concat!(env!("CARGO_MANIFEST_DIR"), "/app")`
- menus.json and plugins.json generated at prebuild time
