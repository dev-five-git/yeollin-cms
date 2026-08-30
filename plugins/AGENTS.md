# PLUGINS - TEMPLATE & EXAMPLES

## OVERVIEW

Plugin examples demonstrating Yeollin architecture. Use as templates for new plugins.

## STRUCTURE (FLAT CRATE LAYOUT)

**NEW**: Plugins now use a flat structure with `Cargo.toml` at root (not in `api/` subdirectory).

```
plugins/
└── example-plugin/    # Library plugin (lib.rs only)
    ├── Cargo.toml     # At root level
    ├── src/           # Rust source
    │   └── lib.rs     # yeollin_plugin! macro
    ├── app/           # Frontend pages with route groups
    │   └── (example)/ # Route group
    ├── package.json   # devDeps for TypeScript DX
    └── tsconfig.json  # Extends packages/app/tsconfig.json
```

Similarly, standalone apps in `apps/` use the same flat structure:

```
apps/
└── example-app/       # Standalone CMS (has main.rs)
    ├── Cargo.toml     # At root level
    ├── src/           # Rust source
    │   └── main.rs    # Entry point
    └── app/           # Frontend pages
```

## TWO PLUGIN PATTERNS

| Pattern | Has main.rs | Use Case |
|---------|-------------|----------|
| **Standalone** (apps/example-app) | Yes | Complete CMS application |
| **Library** (plugins/example-plugin) | No | Reusable plugin crate |

## PLUGIN ANATOMY (NEW FLAT STRUCTURE)

```
my-plugin/
├── Cargo.toml               # Depends on yeollin-plugin (AT ROOT!)
├── src/
│   ├── lib.rs               # yeollin_plugin! macro
│   └── routes/              # Vespera route handlers
├── app/
│   └── (group)/             # App Router route group
│       └── page.tsx         # Frontend pages
├── package.json             # devDeps for TypeScript DX
└── tsconfig.json            # Extends packages/app/tsconfig.json
```

## CREATING A NEW PLUGIN

1. Copy `example-plugin/` structure
2. Update `Cargo.toml` name and dependencies (at root, not in `api/`)
3. Edit `src/lib.rs` with plugin metadata
4. Add routes in `src/routes/`
5. Add frontend in `app/(your-group)/`
6. Run `bun install` from workspace root for TypeScript support

## TYPESCRIPT SETUP

Each plugin has `tsconfig.json` extending `packages/app/tsconfig.json`:
- IDE gets full type support for React, vinext's Next-compatible APIs, @devup-ui/react
- `@/*` paths resolve to `packages/app/src/*`
- devDependencies in package.json provide types locally
- Run `bun run typecheck` to verify types

## CONVENTIONS

- Frontend path: `concat!(env!("CARGO_MANIFEST_DIR"), "/app")` (NOT `/../app`)
- Route groups: `(groupname)/` for URL-hidden segments
- Page discovery: a directory is a route when it contains `page.tsx`
- Menus and access rules: declared in `route.meta.json` next to `page.tsx`

### route.meta.json

```json
{ "label": "Items", "icon": "box", "order": 10, "access": "authenticated", "menu": true }
```

Every field is optional. Defaults: label = directory name, `order` = 50,
`access` = `authenticated`, `menu` = true (false for dynamic segments).

`access` is the ONLY way to make a page reachable without a session. Putting a
page inside `(public)` or `(guest)` grants nothing. Invalid metadata, duplicate
route paths, and unknown fields fail the build instead of falling back.

A plugin's own URL prefix comes from its crate name in `yeollin_plugin!`, not
from the route group, so `app/(memo)/archive/page.tsx` in `example-memo-plugin`
serves `/example-memo-plugin/archive`.

## MIGRATION FROM OLD STRUCTURE

If you have plugins with the old `api/` subdirectory structure:

**Old (deprecated):**
```
my-plugin/
├── api/
│   ├── Cargo.toml
│   └── src/
└── app/
```

**New (current):**
```
my-plugin/
├── Cargo.toml      # Move to root
├── src/            # Move from api/src/
└── app/
```

Steps:
1. Move `api/Cargo.toml` to plugin root
2. Move `api/src/` to plugin root
3. Delete empty `api/` directory
4. Update relative paths in `Cargo.toml` (e.g., `../../crates/plugin`)
5. Update workspace `Cargo.toml` members (remove `/api` suffix)

## ANTI-PATTERNS

- **NO** `api/` subdirectory (use flat structure)
- **NO** `concat!(env!("CARGO_MANIFEST_DIR"), "/../app")` (use `/app`)
- **NO** next.config.* in plugin app/ (marks as complete app)
- **NO** fetch() in page components (use RSC file reads)
- **NO** manual menu.json (generated from page.tsx + route.meta.json)
- **NO** `route.ts` config files (replaced by `route.meta.json`)
- **NO** relying on `(public)` / `(guest)` directory names for access control
