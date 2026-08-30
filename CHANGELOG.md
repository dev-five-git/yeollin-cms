# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The project is pre-1.0, so breaking changes can appear in any release.

## [Unreleased]

### Security

- Authentication middleware no longer exempts any path ending in `.ico`. Only exact-match files such as `/favicon.ico` are exempt.
- Vite dev-server asset paths (`/@`, `/__vite_hmr`, `/node_modules/`, `/src/`, `/df/`) now skip authentication only while the dev proxy is active.
- Public and guest route matching is whole-path exact instead of prefix-based, and the server refuses to start unless `JWT_SECRET` is at least 32 bytes.
- Credentials moved out of the framework into the new `auth-users` plugin. Passwords are verified against Argon2 hashes instead of the previous plaintext comparison against an environment variable.
- Refresh tokens are now opaque random values stored only as SHA-256 hashes. They rotate on every use and can be revoked, so a stolen or replayed token stops working.
- Route access is declared in `route.meta.json` and defaults to `authenticated`. Directory names such as `(public)` no longer grant access, and invalid route metadata fails the build.

### Added

- `auth-users` plugin: users and sessions tables, `/api/auth/login`, `/api/auth/refresh`, `/api/auth/logout`, `/api/auth/me`, and first-administrator bootstrap from `YEOLLIN_ADMIN_USERNAME` / `YEOLLIN_ADMIN_PASSWORD`.
- `route-manifest.json` generated at prebuild alongside `menus.json` and `plugins.json`.

### Changed

- The frontend template migrated from Next.js to vinext.
- Updated `argon2` to 0.6.
- `SuperadminConfig` and `AuthConfig::superadmin` were removed. Applications register the `auth-users` plugin instead.
- Metadata export replaced `YEOLLIN_EXPORT_MENUS` / `YEOLLIN_EXPORT_PLUGINS` with a single `YEOLLIN_EXPORT` envelope on stdout. Application logs must go to stderr.
- `YeollinAppBuilder::with_database_url` connects lazily so metadata exports no longer create a database.
