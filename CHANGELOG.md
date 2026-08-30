# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The project is pre-1.0, so breaking changes can appear in any release.

## [Unreleased]

### Security

- Authentication middleware no longer exempts any path ending in `.ico`. Only exact-match files such as `/favicon.ico` are exempt.
- Vite dev-server asset paths (`/@`, `/__vite_hmr`, `/node_modules/`, `/src/`, `/df/`) now skip authentication only while the dev proxy is active.
- Public and guest route matching is whole-path exact instead of prefix-based, and the server refuses to start unless `JWT_SECRET` is at least 32 bytes.
- Credentials moved out of the framework into the new `auth` plugin. Passwords are verified against Argon2 hashes instead of the previous plaintext comparison against an environment variable.
- Refresh tokens are now opaque random values stored only as SHA-256 hashes. They rotate on every use and can be revoked, so a stolen or replayed token stops working.
- Route access is declared in `route.meta.json` and defaults to `authenticated`. Directory names such as `(public)` no longer grant access, and invalid route metadata fails the build.

- Plugin API routes are mounted under `/api/<name>`, derived from the plugin name and overridable with `api_base`. Handler file location no longer determines the public URL. `/memo` moved to `/api/example-memo-plugin`, `/api/example/items` to `/api/example-plugin/items`, and the `auth-users` plugin was renamed to `auth` so that its routes stay at `/api/auth`.
- OpenAPI component schemas are namespaced by the plugin, so two plugins may each declare a type of the same name.

### Added

- The `audit-log` plugin provides an administrator-only, paginated event history with exact-name filtering and a typed retention setting. It reads audit-marked rows from the transactional event outbox instead of maintaining duplicate storage.
- Events can opt into audit history with `Event::AUDIT`. The default is `false`; memo create, update, and delete events opt in. Retention removes only processed audit rows so pending Deferred delivery remains recoverable.
- Typed event hooks: plugin authors implement `Event` for serializable payloads, emit through an `EventTransaction`, and register exact-name Inline or Deferred subscribers in plugin metadata.
- A transactional `events` outbox persists events with the action, aborts the action when an Inline subscriber fails, and delivers Deferred subscribers after commit through a notify-driven drainer with polling recovery.
- Typed plugin settings: `settings: MySettings` registers a `Serialize + Deserialize + Schema + Default` contract, persists one validated JSON row per plugin, generates administrator-only GET/PUT endpoints, and exposes a `SettingsStore` Axum extension for typed reads.
- Settings pages are generated from the Vespera schema during prebuild. A plugin can replace its generated form with `app/settings/page.tsx`, and the shell settings page now links only to real plugin configuration surfaces.
- Account management in the `auth` plugin: `POST /api/auth/users` creates, `PATCH /api/auth/users/{id}` changes a role, `DELETE /api/auth/users/{id}` removes an account and its sessions, `POST /api/auth/password` changes your own, and `POST /api/auth/users/{id}/password` lets an administrator reset another. Every one is administrator-only except changing your own password, which requires the current one.
- A **Users** page at `/auth`, the plugin's first frontend. Roles are `admin` and `user`; an unrecognised role is refused rather than stored, since role matching is exact and a typo would grant nothing.
- Lockout guards: the only administrator cannot be demoted or deleted, and no account can delete the one it is signed in as. Recovering from either would mean editing the database by hand.
- `yeollin plugin add` registers a plugin with an application, editing both its `Cargo.toml` dependency and the `yeollin_app!` list. `yeollin plugin doctor` reports plugins that are declared on only one side. The workspace `members` list is globbed, so creating a plugin no longer requires editing it.
- `Authorize` on `CurrentUser` (`require_role`, `require_any_role`, `has_role`) for guarding routes that authentication alone does not protect. `GET /api/auth/users` uses it.
- `auth` plugin: users and sessions tables, `/api/auth/login`, `/api/auth/refresh`, `/api/auth/logout`, `/api/auth/me`, and first-administrator bootstrap from `YEOLLIN_ADMIN_USERNAME` / `YEOLLIN_ADMIN_PASSWORD`.

### Changed

- Passwords must be at least 12 **characters** — counted as characters, not bytes, so a short multi-byte password cannot pass. This applies to the bootstrap administrator too, so a deployment whose `YEOLLIN_ADMIN_PASSWORD` is shorter now fails at startup with the reason rather than seeding a weak account.
- Changing or resetting a password ends every session for that account. A refresh token minted before the change stops working, which is what makes a password change useful for containing a compromise.

### Fixed

- Plugin `dependencies` from `package.json` are merged into the generated frontend. Previously only the host application's were, so a plugin's JavaScript dependency was never installed and its pages resolved against whatever the workspace happened to hoist. Conflicting requirements now fail the build instead of resolving silently; `devDependencies` stay local to each crate.
- `api_base` on `yeollin_plugin!` for plugins whose URL namespace should differ from their name. It must not contain `api`, which is always prepended.
- `route-manifest.json` generated at prebuild alongside `menus.json` and `plugins.json`.

### Changed

- The frontend template migrated from Next.js to vinext.
- Updated `argon2` to 0.6.
- `SuperadminConfig` and `AuthConfig::superadmin` were removed. Applications register the `auth` plugin instead.
- Metadata export replaced `YEOLLIN_EXPORT_MENUS` / `YEOLLIN_EXPORT_PLUGINS` with a single `YEOLLIN_EXPORT` envelope on stdout. Application logs must go to stderr.
- `YeollinAppBuilder::with_database_url` connects lazily so metadata exports no longer create a database.
