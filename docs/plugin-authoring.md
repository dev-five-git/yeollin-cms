# Plugin authoring

A plugin is one Rust crate. It bundles its Axum/Vespera API routes, its vinext
frontend pages under `app/`, and optionally sea-orm models with vespertide JSON
migrations. Registering the crate in an application is all it takes to get both
halves.

This guide walks through `plugins/example-memo-plugin`, the reference full-stack
plugin, end to end.

See also: [Getting started](getting-started.md),
[Deployment and security](deployment-and-security.md),
[Architecture overview](architecture.md),
and the [README](../README.md).

## Crate layout

Plugins use a flat layout with `Cargo.toml` at the crate root. There is no `api/`
subdirectory.

```
plugins/example-memo-plugin/
?��??� Cargo.toml                          # crate manifest, at the root
?��??� package.json                        # devDependencies for TypeScript DX
?��??� tsconfig.json                       # extends packages/app/tsconfig.json
?��??� vespertide.json                     # database model + migration config
?��??� models/
??  ?��??� memo.json                       # model definition
?��??� migrations/
??  ?��??� 0001_initial.vespertide.json    # generated migration
?��??� src/
??  ?��??� lib.rs                          # yeollin_plugin! macro
??  ?��??� models/
??  ??  ?��??� mod.rs
??  ??  ?��??� memo.rs                     # generated sea-orm entity
??  ?��??� routes/
??      ?��??� mod.rs
??      ?��??� memo.rs                     # Vespera route handlers
?��??� app/
    ?��??� (memo)/
        ?��??� page.tsx                    # frontend page
        ?��??� route.meta.json             # label, order, access, menu
```

Standalone applications under `apps/` use the same layout, with `src/main.rs`
instead of `src/lib.rs`.

## `Cargo.toml`

```toml
[package]
name = "example-memo-plugin"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Example memo plugin with database CRUD for Yeollin CMS"

[dependencies]
yeollin-plugin = { path = "../../crates/plugin" }
vespera = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sea-orm = { workspace = true }
axum = { workspace = true }
chrono = { workspace = true }
vespertide = { workspace = true }
anyhow = { workspace = true }
```

Shared dependencies come from the workspace root so every crate builds against
one version. `yeollin-plugin` is the only Yeollin crate a plugin needs; it
re-exports what the macro expands to.

A plugin without a database can drop `sea-orm`, `chrono`, and `vespertide`. See
`plugins/example-plugin/Cargo.toml` for the minimal set.

## `src/lib.rs` and the `yeollin_plugin!` macro

```rust
pub mod models;
pub mod routes;

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    author: "DevFive",
    description: "Example memo plugin with database CRUD operations",
}

// Re-export entity for migrations
pub use models::memo;
```

Declaring the `routes` module is what pulls the route handlers into the crate.
The macro expands to `vespera::export_app!` plus a `metadata()` function, so the
handlers annotated with `#[vespera::route(...)]` anywhere under `src/` are
collected automatically.

### Macro fields

| Field | Required | Type | Meaning |
|-------|----------|------|---------|
| `name` | yes | string literal | The plugin name. Also becomes its frontend URL prefix. |
| `author` | no | string literal | Recorded in the exported plugin metadata. |
| `description` | no | string literal | Recorded in the exported plugin metadata. |
| `on_init` | no | expression | An `async fn(DatabaseConnection) -> anyhow::Result<()>` run once at startup. |
| `frontend` | no | bool literal | `true` by default. Set `false` for an API-only plugin with no `app/` directory. |
| `api_base` | no | string literal | Override the API namespace derived from `name`; never include `api`. |
| `settings` | no | Rust type path | Register a typed settings contract and its generated API and page. |

Anything else is a compile error: the macro rejects unknown fields.

The version and licence come from the crate manifest via `CARGO_PKG_VERSION` and
`CARGO_PKG_LICENSE`, so there is nothing to keep in sync by hand.

### Export identifier

The macro derives a PascalCase export identifier from `name`:
`"example-memo-plugin"` becomes `ExampleMemoPlugin`. The `yeollin_app!` macro
derives the same identifier from the plugin's module path
(`example_memo_plugin`). Hyphens and underscores collapse identically, so the two
always agree.

### `on_init` and vespertide

If you do not pass `on_init` and a `vespertide.json` file exists next to the
crate manifest, the macro generates one for you that runs
`vespertide::vespertide_migration!(&db)`. That is why `example-memo-plugin` never
writes an initialiser: its migrations are applied automatically at startup.

Pass `on_init` explicitly when you need more. `auth` does exactly that, to
run migrations *and* seed the first administrator:

```rust
yeollin_plugin::yeollin_plugin! {
    name: "auth",
    author: "DevFive",
    description: "Database-backed users and sessions",
    on_init: initialize,
    frontend: false,
}

async fn initialize(db: DatabaseConnection) -> anyhow::Result<()> {
    vespertide::vespertide_migration!(&db).await?;
    seed_first_admin(&db).await
}
```

`frontend: false` there means the crate ships no `app/` directory at all.

An `on_init` callback runs only when the application has a database configured.
If it returns an error, startup fails with the plugin name attached.

## Typed plugin settings

Declare a settings struct before `yeollin_plugin!` and pass its type to the
`settings` field:

```rust
use serde::{Deserialize, Serialize};
use vespera::Schema;

#[derive(Debug, Default, Serialize, Deserialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MemoSettings {
    pub compact_mode: bool,
    pub footer_note: String,
}

yeollin_plugin::yeollin_plugin! {
    name: "example-memo-plugin",
    settings: MemoSettings,
}
```

The macro adds administrator-only `GET` and `PUT` handlers at
`/api/example-memo-plugin/settings`. Values are deserialized through
`MemoSettings` before they are written, and the framework stores exactly one
JSON row for the plugin. Existing values survive restarts; `Default` seeds a
new installation.

Plugin handlers can read the same value without parsing untyped JSON:

```rust
use axum::Extension;
use yeollin_plugin::SettingsStore;

async fn render(Extension(settings): Extension<SettingsStore>) -> anyhow::Result<()> {
    let settings = settings
        .get::<MemoSettings>("example-memo-plugin")
        .await?;
    // use settings.compact_mode
    Ok(())
}
```

Prebuild serializes Vespera's schema and `Default` into `plugins.json` and
generates `/<plugin>/settings`. To own the presentation, add
`app/settings/page.tsx`; that exact file replaces the generated page while the
typed API and persistence stay unchanged. Do not place the override under a
route group.

## API routes: how URLs are derived

Every plugin API lives under `/api/<base>`:

1. **`<base>` comes from the plugin declaration** ??the `name`, or `api_base` when
   you set it. `/api` is always prepended and must not appear in `api_base`.
2. **The module path under `src/routes/`** is appended to that namespace.
3. **The macro's `path` argument** is appended last.

The frontend already derives its pages from the same `name`, so one declaration
gives `/<name>` for pages and `/api/<name>` for the API.

```rust
yeollin_plugin! { name: "media-library" }
// pages at /media-library, API under /api/media-library
```

Set `api_base` only when the URL namespace should differ from the name:

```rust
yeollin_plugin! { name: "reporting-suite", api_base: "reports" }  // -> /api/reports
```

Underscores become hyphens, because `-` is the URL convention: `media_library`
and `media-library` both produce `/api/media-library`.

### Placing handlers

Handlers in `src/routes/mod.rs` sit at the namespace root, which is usually what
you want. A file adds its own segment.

For a plugin whose base resolves to `/api/example-plugin`:

| Handler location | `path` | URL |
|---|---|---|
| `src/routes/mod.rs` | omitted | `/api/example-plugin` |
| `src/routes/mod.rs` | `/{id}` | `/api/example-plugin/{id}` |
| `src/routes/items.rs` | `/` | `/api/example-plugin/items/` |
| `src/routes/items.rs` | `/{id}` | `/api/example-plugin/items/{id}` |

`example-memo-plugin` keeps all five CRUD handlers in `src/routes/mod.rs`, so
they land on `/api/example-memo-plugin` and `/api/example-memo-plugin/{id}`.

Do **not** nest a `src/routes/api/<name>/` directory. The namespace is already
prepended, so that would produce `/api/<base>/api/<name>/...`.

Because the URL comes from the declaration rather than the file tree, moving a
handler between files does not change its published endpoint.

### A handler

Request and response bodies derive `vespera::Schema` so they appear in the
generated OpenAPI document, alongside serde:

```rust
#[derive(Serialize, Schema)]
#[serde(rename_all = "camelCase")]
pub struct MemoResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

/// List all memos
#[vespera::route(get, tags = ["memo"])]
pub async fn list_memos(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<ListMemosResponse>, (StatusCode, Json<ErrorResponse>)> {
    let memos = memo::Entity::find()
        .order_by(memo::Column::CreatedAt, Order::Desc)
        .all(&db)
        .await
        .map_err(/* ... */)?;
    // ...
}
```

The database connection arrives as an Axum `Extension`, installed by the runtime
when the application is configured with a database. `tags` groups the operation
in the OpenAPI document. The doc comment above the handler becomes its summary.

Do not block inside an async handler.

## Guarding routes by role

The auth middleware establishes *who* is calling. It does not decide what they
may do, so a protected route is reachable by every signed-in account until the
handler says otherwise. Ask for the role you need:

```rust
use yeollin_plugin::{Authorize, CurrentUser, PluginError};

/// Remove a memo. Administrators only.
#[vespera::route(delete, path = "/{id}", tags = ["memo"])]
pub async fn delete_memo(
    Extension(db): Extension<DatabaseConnection>,
    Extension(current): Extension<CurrentUser>,
    Path(id): Path<i32>,
) -> Result<Json<DeleteResponse>, PluginError> {
    current.require_role("admin")?;
    // ...
}
```

`require_any_role(&["admin", "editor"])` accepts several, and `has_role` returns
a `bool` when you want to vary a response rather than refuse it.

Refusals are always a plain `403 FORBIDDEN`, whichever role was missing, so
probing endpoints cannot map out the role model. Matching is exact: `Admin` and
`admin` are different roles.

Audit administrative and destructive endpoints for one of these calls. Forgetting
one leaves the endpoint open to any authenticated user, which no test will catch
unless you write it. `GET /api/auth/users` in the `auth` plugin is a worked
example.

A role check is not a sandbox. Plugins are statically linked and run with full
process privileges, so this enforces *user* authorization, not isolation of
plugin code.

## Frontend pages

Pages live under `app/` in the plugin crate and follow the App Router
conventions. A directory becomes a route when it contains `page.tsx`.

```
app/
?��??� (memo)/
    ?��??� page.tsx
    ?��??� route.meta.json
```

A plugin's frontend URL prefix comes from the `name` field in `yeollin_plugin!`,
not from the directory tree. Directories wrapped in parentheses are route groups:
they organise files and contribute no URL segment. So `example-memo-plugin`'s
`app/(memo)/page.tsx` serves `/example-memo-plugin`, and an
`app/(memo)/archive/page.tsx` would serve `/example-memo-plugin/archive`.

The macro resolves the directory as
`concat!(env!("CARGO_MANIFEST_DIR"), "/app")`. With the flat layout that is
`/app`, never `/../app`.

Pages are plain React. `example-memo-plugin` uses `@devup-ui/react` primitives
and calls its own API with `fetch`:

```tsx
'use client'

import { Box, Flex, Text, VStack } from '@devup-ui/react'

async function fetchMemoList(): Promise<Memo[]> {
  const response = await fetch('/api/example-memo-plugin')
  if (!response.ok) {
    throw new Error('Failed to fetch memos')
  }
  const data = await response.json()
  return data.memos || []
}
```

Typography tokens are `heading`, `subheading`, `body`, and `label`.

Do not put a `next.config.*` file in a plugin's `app/`: that marks the directory
as a complete application rather than a set of pages to merge.

### `route.meta.json`

Menu placement and access rules are declared in a `route.meta.json` sidecar next
to `page.tsx`. There are no `route.ts` config files.

```json
{
  "label": "Memo",
  "order": 20
}
```

| Field | Type | Default | Meaning |
|-------|------|---------|---------|
| `label` | string | the last URL segment | Display name in the menu. |
| `icon` | string | none | Icon name carried into the menu entry. |
| `order` | integer | `50` | Sort key. Lower sorts first. |
| `access` | `"authenticated"` \| `"public"` \| `"guest"` | `"authenticated"` | Who may reach the page. |
| `menu` | boolean | `true`, or `false` for a dynamic route | Whether the page appears in navigation. |

The file is optional; omitting it takes every default.

Rules worth internalising:

- `access` is the only way to make a page reachable without a session. Putting a
  page inside `(public)` or `(guest)` grants nothing at all.
- `menu` affects navigation only, never authorization.
- Unknown fields are rejected. A misspelled `acess` fails the build instead of
  silently leaving the route on its default.
- Duplicate route paths fail the build, and the diagnostic names the plugin that
  claimed the path first.
- `menu: true` on a route with a dynamic segment fails the build.
- Every diagnostic is reported in one pass, sorted deterministically, so a build
  surfaces all broken sidecars at once.

`apps/example-app` has a working example at
`app/(public)/test/route.meta.json`:

```json
{
  "access": "public",
  "menu": false
}
```

## Database models

Three pieces work together: `vespertide.json` configures the generator,
`models/*.json` declare the tables, and `migrations/` holds the generated
migration files that run at startup.

### `vespertide.json`

```json
{
  "modelsDir": "models",
  "migrationsDir": "migrations",
  "tableNamingCase": "snake",
  "columnNamingCase": "snake",
  "modelFormat": "json",
  "migrationFormat": "json",
  "migrationFilenamePattern": "%04v_%m",
  "modelExportDir": "src/models",
  "seaorm": {
    "extraEnumDerives": ["vespera::Schema"],
    "extraModelDerives": [],
    "enumNamingCase": "camel"
  },
  "prefix": "memp_"
}
```

`modelExportDir` is where the generated sea-orm entities land, which is why
`src/models/memo.rs` exists but is not written by hand. `prefix` namespaces the
plugin's tables so two plugins can both own a table called `memos` without
colliding. `auth` uses the prefix `auth_`.

The presence of this file is also what makes `yeollin_plugin!` generate an
`on_init` that applies migrations.

### `models/memo.json`

```json
{
  "$schema": "https://raw.githubusercontent.com/dev-five-git/vespertide/refs/heads/main/schemas/model.schema.json",
  "name": "memos",
  "description": "Memo storage for example-memo-plugin",
  "columns": [
    { "name": "id", "type": "integer", "nullable": false,
      "primary_key": { "auto_increment": true } },
    { "name": "title", "type": "text", "nullable": false },
    { "name": "content", "type": "text", "nullable": false },
    { "name": "created_at", "type": "timestamptz", "nullable": false,
      "default": "NOW()", "index": true },
    { "name": "updated_at", "type": "timestamptz", "nullable": false,
      "default": "NOW()" }
  ]
}
```

Add a `constraints` array for uniqueness. `auth` uses it to keep usernames
and refresh-token hashes unique:

```json
"constraints": [
  { "type": "unique", "name": "uq_auth_users_username", "columns": ["username"] }
]
```

### `migrations/`

Migration files are numbered by the `migrationFilenamePattern`, so the memo
plugin's first one is `0001_initial.vespertide.json`. Each carries a `version`,
a `comment`, a `created_at`, and a list of `actions`:

```json
{
  "$schema": "https://raw.githubusercontent.com/dev-five-git/vespertide/refs/heads/main/schemas/migration.schema.json",
  "actions": [
    {
      "type": "create_table",
      "table": "memos",
      "columns": [ /* ... */ ],
      "constraints": []
    }
  ],
  "comment": "Initial",
  "created_at": "2026-01-23T18:53:44Z",
  "version": 1
}
```

Migrations are committed to the repository. `vespertide_migration!` applies them
during plugin initialisation, so a fresh SQLite file gets its schema on first
run.

Re-export the entity from `lib.rs` so migrations and downstream crates can reach
it:

```rust
pub use models::memo;
```

## TypeScript setup

Each plugin carries a `package.json` and a `tsconfig.json` purely for editor and
typecheck support. The plugin is not published as a Node package.

```json
{
  "extends": "../../packages/app/tsconfig.json",
  "compilerOptions": {
    "paths": { "@/*": ["../../packages/app/src/*"] },
    "noEmit": true
  },
  "include": ["app/**/*.ts", "app/**/*.tsx"],
  "exclude": ["node_modules"]
}
```

Run `bun install` once from the repository root, then typecheck the plugin with:

```bash
cd plugins/example-memo-plugin
bun x tsc --noEmit
```

## Register the plugin in an application

```bash
cd apps/example-app
yeollin plugin add my-plugin
yeollin plugin doctor
```

Cargo resolves the dependency graph before proc macros run, so a plugin cannot be
discovered at compile time. The host application must declare it in two places,
and half a registration either fails to compile or silently omits the plugin's
routes and migrations. `plugin add` makes both edits and is a no-op when re-run;
`plugin doctor` reports a plugin declared on only one side and exits non-zero, so
it can gate CI.

Both edits are shown below, since you will read them in existing applications.

First, the dependency in `Cargo.toml`:

```toml
[dependencies]
yeollin-app = { path = "../../crates/app" }
example-plugin = { path = "../../plugins/example-plugin" }
example-memo-plugin = { path = "../../plugins/example-memo-plugin" }
auth = { path = "../../plugins/auth" }
```

Then add its module path to the `plugins` list in `yeollin_app!`. Note the
underscored form: the crate name `example-memo-plugin` is the module
`example_memo_plugin`.

```rust
let app = yeollin::yeollin_app! {
    plugins: [auth, example_plugin, example_memo_plugin],
    openapi: "openapi.json",
    title: "Example CMS API",
    version: "1.0.0",
    docs_url: "/docs",
    redoc_url: "/redoc",
}
.host("0.0.0.0")
.port(port)
.with_auth(auth_config)
.with_database_url("sqlite://./db.sqlite?mode=rwc")
.build();

app.run().await
```

`yeollin_app!` does three things per plugin: calls `register_plugin(metadata())`,
merges the plugin's Vespera routes into the application's OpenAPI document, and
registers the host application's own `app/` directory as a route source. Plugin
order in the list is the order they are registered.

Prefer `with_database_url` over `with_database`. It connects lazily, which keeps
metadata export free of side effects.

## The metadata export contract

`prebuild` learns what a binary contains by running it once with
`YEOLLIN_EXPORT=1`. There is a single export variable; the binary responds with
one `ExportEnvelope` JSON document containing its `schemaVersion`, `plugins`,
`menus`, and `routes`.

If you write your own `main.rs`, three rules apply:

1. Emit exactly one `ExportEnvelope` on **stdout** and nothing else.
2. Send all logs to **stderr**. `apps/example-app` does this with
   `tracing_subscriber::fmt::layer().with_writer(std::io::stderr)`.
3. Do no work before the export branch. `YeollinApp::run` handles the export
   first, before validating the JWT secret, before connecting to a database, and
   before running any `on_init`, so metadata export needs no deployment secrets.

## Creating a new plugin

```bash
yeollin init my-plugin
cd apps/example-app
yeollin plugin add my-plugin
bun install                       # from the repository root
```

`init` scaffolds the crate and `plugin add` registers it. The workspace `members`
list is globbed (`crates/*`, `plugins/*`, `apps/*`), so it needs no edit. Then
fill in the crate:

1. Edit `src/lib.rs` with your `yeollin_plugin!` metadata.
2. Add handlers under `src/routes/`, choosing the module path that gives the URL
   base you want, and a role check on anything administrative or destructive.
3. Add pages under `app/(your-group)/`, with a `route.meta.json` beside each
   `page.tsx` that needs a label, an order, or a non-default access rule.
4. If you need tables, add `vespertide.json`, `models/*.json`, and the generated
   `migrations/`.
5. Declare any JavaScript your pages import in the plugin's `package.json`
   `dependencies`. Prebuild merges those into the assembled app;
   `devDependencies` stay local to the crate.

`plugins/example-plugin/` is a minimal reference and `plugins/example-memo-plugin/`
adds a database.

## Anti-patterns

- No `api/` subdirectory. `Cargo.toml` goes at the crate root.
- No `concat!(env!("CARGO_MANIFEST_DIR"), "/../app")`. Use `/app`.
- No `next.config.*` inside a plugin's `app/`.
- No `route.ts` config files. Use `route.meta.json`.
- No relying on `(public)` or `(guest)` directory names for access control.
- No hand-written `menus.json`. It is generated from `page.tsx` plus
  `route.meta.json`.
- No blocking calls in async handlers.
- No hardcoded filesystem paths. Use `CARGO_MANIFEST_DIR`.

## Next

Before you deploy anything you built here, read
[Deployment and security](deployment-and-security.md).
