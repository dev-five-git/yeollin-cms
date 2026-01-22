# PLUGINS - TEMPLATE & EXAMPLES

## OVERVIEW

Plugin examples demonstrating Yeollin architecture. Use as templates for new plugins.

## STRUCTURE

```
plugins/
├── example-app/       # Standalone CMS (has main.rs)
│   ├── api/          # Rust crate with main.rs + lib.rs
│   └── app/          # Frontend pages
└── example-plugin/    # Library plugin (lib.rs only)
    ├── api/          # Rust crate with lib.rs
    └── app/          # Frontend pages with route groups
```

## TWO PLUGIN PATTERNS

| Pattern | Has main.rs | Use Case |
|---------|-------------|----------|
| **Standalone** (example-app) | Yes | Complete CMS application |
| **Library** (example-plugin) | No | Reusable plugin crate |

## PLUGIN ANATOMY

```
my-plugin/
├── api/
│   ├── Cargo.toml           # Depends on yeollin-plugin
│   └── src/
│       ├── lib.rs           # yeollin_plugin! macro
│       └── routes/          # Vespera route handlers
├── app/
│   └── (group)/             # Next.js route group
│       └── page.tsx         # Frontend pages
├── package.json             # devDeps for TypeScript DX
└── tsconfig.json            # Extends packages/app/tsconfig.json
```

## CREATING A NEW PLUGIN

1. Copy `example-plugin/` structure
2. Update `api/Cargo.toml` name and dependencies
3. Edit `api/src/lib.rs` with plugin metadata
4. Add routes in `api/src/routes/`
5. Add frontend in `app/(your-group)/`
6. Run `bun install` from workspace root for TypeScript support

## TYPESCRIPT SETUP

Each plugin has `tsconfig.json` extending `packages/app/tsconfig.json`:
- IDE gets full type support for React, Next.js, @devup-ui/react
- `@/*` paths resolve to `packages/app/src/*`
- devDependencies in package.json provide types locally
- Run `bun run typecheck` to verify types

## CONVENTIONS

- Frontend path: `concat!(env!("CARGO_MANIFEST_DIR"), "/../app")`
- Route groups: `(groupname)/` for URL-hidden segments
- Page discovery: prebuild scans for `page.tsx` files
- Menus: auto-generated from route structure

## ANTI-PATTERNS

- **NO** next.config.* in plugin app/ (marks as complete app)
- **NO** fetch() in page components (use RSC file reads)
- **NO** manual menu.json (generated from page.tsx locations)
