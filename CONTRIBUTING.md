# Contributing

Thanks for working on Yeollin CMS. This guide covers the checks you need to
pass, the layout rules for new crates, and how dependencies are declared.

## Before you open a pull request

Run the same commands CI runs. If they pass locally, CI should agree.

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

Notes:

- Clippy runs with `-D warnings`, so a warning is a failure.
- `bun run lint` invokes oxlint. oxlint reads `oxlint.config.ts` through Node,
  so you need Node installed even though the rest of the toolchain uses Bun.
- The typecheck loop covers four packages. If you add a package with its own
  `tsconfig.json`, add it to the CI list too.
- The `--skip-backend` build exercises prebuild (template extraction, plugin
  frontend assembly, the menus and plugins manifests) and the vinext static
  export. CI only builds the release binary on `main`.

## Crate layout for new plugins and apps

Plugins and standalone apps share one flat layout. `Cargo.toml` lives at the
crate root. There is no `api/` subdirectory; that shape is legacy and should not
be used for anything new.

```
my-plugin/            # or my-app/
├── Cargo.toml        # at the crate root
├── src/
│   ├── lib.rs        # plugin: yeollin_plugin!
│   └── main.rs       # app: entry point
├── app/              # vinext frontend pages
│   └── (group)/
├── package.json      # Node devDependencies for TypeScript DX
└── tsconfig.json     # extends packages/app/tsconfig.json
```

To add one:

1. Copy the structure of `plugins/example-plugin` (or `apps/example-app` for a
   standalone app).
2. Set the crate name in `Cargo.toml`, keeping the file at the crate root.
3. Declare the plugin metadata in `src/lib.rs` with `yeollin_plugin!`.
4. Add Vespera route handlers under `src/`.
5. Add frontend pages under `app/(your-group)/`.
6. Add the crate path to `members` in the root `Cargo.toml`, and run
   `bun install` from the repository root so the Node workspace picks it up.

Point the plugin's frontend at `concat!(env!("CARGO_MANIFEST_DIR"), "/app")`,
not at a path that walks up a directory.

## Dependencies

Declare every shared dependency once, in `[workspace.dependencies]` in the root
`Cargo.toml`, and reference it from member crates:

```toml
[dependencies]
axum = { workspace = true }
serde = { workspace = true }
yeollin-plugin = { workspace = true }
```

Do not pin a version in a member crate when the root workspace already declares
it. Bumping a version should be a one-line change at the root.

## Breaking changes

The project is pre-1.0. Breaking changes are acceptable, and you do not need to
build a compatibility shim for them. What you do need to do is record them:
add an entry to the `## [Unreleased]` section of [CHANGELOG.md](CHANGELOG.md)
under the appropriate subsection, describing what changed and what a consumer
has to do differently.
