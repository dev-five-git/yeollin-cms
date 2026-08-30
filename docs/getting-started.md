# Getting started

This guide takes you from a fresh clone to a running Yeollin CMS instance you can
sign in to, and then to a release binary.

See also: [Architecture overview](architecture.md),
[Plugin authoring](plugin-authoring.md),
[Deployment and security](deployment-and-security.md),
and the [README](../README.md).

> Yeollin CMS is v0.1 and pre-release. Paths, flags, and public interfaces change
> without notice.

## Prerequisites

| Tool | Why |
|------|-----|
| Rust (stable) | The workspace crates target `edition = "2021"`. CI runs `cargo clippy`, so install the `clippy` component. |
| Bun | Manages the Node workspace and runs the frontend scripts. |
| Node | oxlint loads `oxlint.config.ts` through Node rather than Bun. |

## Install the Node workspace

From the repository root:

```bash
bun install
```

This installs the workspace packages, including the `packages/app` frontend
template. `packages/app` is a template: it is extracted into `.yeollin/app/` at
prebuild time and is never the app you run directly.

## Run the development server

`apps/example-app` is the standalone application used for local development. It
registers `auth`, `audit-log`, `media`, `example-plugin`, and
`example-memo-plugin` through the `yeollin_app!` macro.

```bash
cd apps/example-app
cargo run -p yeollin-cli -- dev
```

Open http://localhost:3001.

### What `dev` does

1. Builds the app crate with `cargo build`.
2. Runs the built binary once with `YEOLLIN_EXPORT=1` to read its plugin, menu,
   and route metadata.
3. Runs prebuild, which extracts the frontend template into `.yeollin/app/` and
   merges each plugin's `app/` pages into it.
4. Installs the frontend dependencies inside `.yeollin/app/` if they are missing.
5. Starts the vinext dev server and the Rust API server.

Everything is served on a single port. The Axum router answers the API routes and
proxies everything else to the internal vinext dev server on port 3000, including
the Vite HMR WebSocket at `/__vite_hmr`. You only ever talk to port 3001.

By default `dev` uses proxy mode, which re-exports plugin pages from their
original source files so HMR is instant. A file watcher picks up added and
deleted pages and restarts the Rust server when route metadata changes.

| Flag | Default | Effect |
|------|---------|--------|
| `--port` | `3001` | The single entry-point port. |
| `--internal-frontend-port` | `3000` | Port the vinext dev server listens on behind the proxy. |
| `--copy-mode` | off | Copies plugin pages instead of re-exporting them. Disables instant HMR. |
| `--skip-prebuild` | off | Reuses the existing `.yeollin/app/` tree. |

`.yeollin/` is generated and gitignored. Never edit it by hand; prebuild
regenerates it.

## `JWT_SECRET`

The server signs access tokens with `JWT_SECRET` and refuses to serve traffic if
the secret is shorter than 32 bytes. The check runs in `YeollinApp::run` before
any request is handled, so a weak secret is a startup failure rather than a
forgeable session.

For local work you do not have to set anything: when `JWT_SECRET` is unset,
`yeollin dev` mints an ephemeral 48-byte secret for that dev session only. Every
restart produces a new secret, which invalidates previously issued tokens.

To keep your session across restarts, export your own:

```bash
export JWT_SECRET="$(openssl rand -base64 48)"
```

A deployed binary is never given a secret automatically. See
[Deployment and security](deployment-and-security.md).

## Create the first administrator

The framework itself has no credential store. the `auth` plugin owns users
and sessions, and it seeds the first administrator from the environment, but only
while the users table is still empty:

| Variable | Purpose |
|----------|---------|
| `YEOLLIN_ADMIN_USERNAME` | Username for the first administrator. Trimmed and lowercased. |
| `YEOLLIN_ADMIN_PASSWORD` | Its password, at least 12 characters. Hashed with Argon2 on insert; never stored or compared in plaintext. |

Set both and start the dev server:

```bash
cd apps/example-app
YEOLLIN_ADMIN_USERNAME=admin YEOLLIN_ADMIN_PASSWORD='a-long-local-password' \
  cargo run -p yeollin-cli -- dev
```

The account is created with the role `admin`. Once any user exists the seed is
skipped entirely, so these variables cannot reset an existing account or bring
back a deleted one.

If no users exist and the two variables are unset, the plugin logs a warning that
nobody can sign in, and startup continues. Set both, restart, and try again.

The database is a SQLite file. `apps/example-app` uses
`sqlite://./db.sqlite?mode=rwc`, so the file is created next to the working
directory on the first run and the schema is provisioned by vespertide during
framework and plugin initialisation.

## Sign in

Go to http://localhost:3001/signin and enter the administrator credentials. The
page posts them to `/api/auth/login` and stores the returned `access_token` and
`refresh_token` in cookies.

`/signin` is a guest route: once you hold a valid access token, requesting it
redirects you to `/`. Every page that does not explicitly say otherwise requires
a session.

The auth endpoints owned by `auth`:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/auth/login` | POST | Exchange a username and password for a token pair. |
| `/api/auth/refresh` | POST | Exchange a refresh token for a new pair, revoking the presented one. |
| `/api/auth/logout` | POST | Revoke the presented refresh token. |
| `/api/auth/me` | GET | Describe the caller behind the access token. |

`login`, `refresh`, and `logout` are reachable without an access token; `me` is
not.

Open **Settings** after signing in. The example plugin demonstrates a custom
settings screen, while the memo plugin's form is generated from its Rust and
Vespera settings schema. Both save through administrator-only typed endpoints.

Create a memo, then open **Audit log**. The memo's `memo.created` event appears
because that event type opts into audit history. Filter values are exact event
names, not prefixes. The generated **audit-log settings** page controls retention
(90 days by default); both the history API and its setting require the `admin`
role.

Open **Media library** to upload a JPEG, PNG, GIF, or WebP image. The file is
written below `./storage/media/objects/`, not into the embedded frontend bundle,
and the UI returns an opaque `media:<32-lowercase-hex>` reference. Content stores
that reference rather than a filesystem path or deployment URL. The generated
**media settings** page controls the upload limit from 1 through 10 MiB (5 MiB
by default). Listing, upload, settings, and delete require `admin`; the fixed
`/api/media/file?reference=...` serving path is public so published pages can
render referenced images.

## Explore the API docs

`apps/example-app` configures Vespera to publish its merged OpenAPI document,
with Swagger UI at `/docs` and ReDoc at `/redoc`. Both sit behind the auth
middleware like any other route, so sign in first.

## Build a release binary

```bash
cd apps/example-app
cargo run -p yeollin-cli -- build
```

The build runs in four stages:

1. `cargo build` produces a binary that answers `YEOLLIN_EXPORT=1` with its
   plugin, menu, and route metadata.
2. `yeollin prebuild` extracts the `packages/app` template into `.yeollin/app/`,
   merges each plugin's `app/` pages, and writes `menus.json`, `plugins.json`,
   and the route manifest.
3. vinext statically exports to `.yeollin/app/dist/client/`, and the CLI copies
   the client output to `.yeollin/app/out/`.
4. `cargo build --release` produces the final binary, embedding the exported
   static files with `include_dir!`.

Pass `--skip-backend` to stop after the frontend export. That is what CI does on
pull requests.

The result is one executable that serves both the API and the UI. Give it a real
`JWT_SECRET` and read
[Deployment and security](deployment-and-security.md) before you run it anywhere
that matters.

## Run the checks CI runs

From the repository root, in this order:

```bash
bun install --frozen-lockfile

cargo clippy --workspace --all-targets --all-features -- -D warnings

cargo test --workspace

RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

bun run lint

for pkg in packages/app apps/example-app plugins/*/; do
  [ -f "$pkg/tsconfig.json" ] || continue
  (cd "$pkg" && bun x tsc --noEmit)
done
```

```bash
# from apps/example-app
cargo run -p yeollin-cli -- build --skip-backend
```

CI additionally runs `cargo build --release -p example-app`, but only on `main`.

## Next

Write your own plugin: [Plugin authoring](plugin-authoring.md).
