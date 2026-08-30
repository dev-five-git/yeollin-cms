# Yeollin CMS

Yeollin CMS is a Tauri-inspired, plugin-based CMS *framework* rather than a finished CMS product. A plugin is a single Rust crate that bundles its Axum/Vespera API routes, a vinext (Vite + RSC, Next-compatible) frontend under `app/`, and optionally sea-orm models with vespertide JSON migrations. The `yeollin-cli` binary assembles the registered plugins into one application: it extracts the frontend template, merges each plugin's pages, statically exports the result, and embeds it into the release binary via `include_dir!`, so a deployment is a single executable that serves both the API and the UI.

## Status

> **Status: v0.1, pre-release. APIs change without notice. Not production ready.**
>
> This repository is private and under active development. Treat every public
> interface, file layout, and CLI flag as unstable.

## Prerequisites

| Tool | Notes |
|------|-------|
| Rust (stable) | The workspace targets `edition = "2021"`. `cargo clippy` is used in CI, so install the `clippy` component. |
| Bun | Node workspace manager and script runner (`bun install`, `bun run lint`). |
| Node | Required by oxlint, which loads `oxlint.config.ts` through Node rather than Bun. |

## Repository map

| Path | Contents |
|------|----------|
| `crates/` | The Rust workspace crates: `core` (shared types), `auth` (JWT, Argon2, middleware), `plugin` (`PluginMetadata`, `FrontendAssets`), `plugin-macros` (`yeollin_plugin!`, `yeollin_app!`), `app` (`YeollinAppBuilder` runtime), `cli` (`init`, `prebuild`, `dev`, `build`). |
| `packages/` | The Node workspace. `packages/app` is the vinext frontend template that gets extracted into `.yeollin/app/` at prebuild time. It is a template, not the running app. |
| `plugins/` | Plugin crates. `example-plugin` is a minimal library plugin; `example-memo-plugin` adds sea-orm models and vespertide migrations. |
| `apps/` | Standalone application crates. `apps/example-app` wires the example plugins together with `yeollin_app!` and is the entry point used for local development. |

`.yeollin/` is generated during prebuild and is gitignored. Never edit it by hand.

## Quick start

```bash
bun install
cd apps/example-app
cargo run -p yeollin-cli -- dev
```

`dev` serves everything on a single port (3001). The Axum router handles the API
routes and proxies everything else to an internal vinext dev server on port
3000, including the Vite HMR WebSocket at `/__vite_hmr`.

### Environment variables

| Variable | Purpose |
|----------|---------|
| `PORT` | API server port. `apps/example-app` defaults to 3001. |
| `JWT_SECRET` | Signing secret for auth tokens. The server refuses to start unless it is at least 32 bytes. |
| `YEOLLIN_ADMIN_USERNAME` / `YEOLLIN_ADMIN_PASSWORD` | Read once by the `auth-users` plugin to create the first administrator while the users table is empty. The password is stored as an Argon2 hash and is never compared in plaintext. |
| `YEOLLIN_DEV_PROXY` | Enables the dev proxy to the vinext port. |
| `YEOLLIN_EXPORT_PLUGINS` / `YEOLLIN_EXPORT_MENUS` | Make the binary print plugin or menu metadata and exit, which is how prebuild discovers them. |

## Build

```bash
cd apps/example-app
cargo run -p yeollin-cli -- build
```

The build runs in four stages:

1. `cargo build` produces a binary that can export plugin and menu metadata.
2. `yeollin prebuild` extracts the `packages/app` template into `.yeollin/app/`, copies each plugin's `app/` pages in, and writes `menus.json` and `plugins.json`.
3. `vinext build` statically exports to `.yeollin/app/dist/client/`, and the CLI copies the client output to `.yeollin/app/out/`.
4. `cargo build --release` produces the final binary, embedding the static files via `include_dir!`.

Pass `--skip-backend` to stop after the frontend export, which is what CI does on
pull requests.

## Local checks

These are exactly the commands CI runs, in order. Run them from the repository
root unless noted.

```bash
bun install --frozen-lockfile

cargo clippy --workspace --all-targets --all-features -- -D warnings

cargo test --workspace

bun run lint

for pkg in packages/app apps/example-app plugins/example-plugin plugins/example-memo-plugin; do
  (cd "$pkg" && bun x tsc --noEmit)
done
```

```bash
# from apps/example-app
cargo run -p yeollin-cli -- build --skip-backend
```

CI additionally builds the release binary with
`cargo build --release -p example-app`, but only on `main`.

## Further reading

- [Architecture overview](docs/architecture.md)
- [Contributing guide](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Changelog](CHANGELOG.md)

## License

MIT. See [LICENSE](LICENSE).
