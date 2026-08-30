# Deployment and security

What a Yeollin CMS deployment needs, what the framework guarantees, and what it
does not guarantee yet.

See also: [Getting started](getting-started.md),
[Plugin authoring](plugin-authoring.md),
[Architecture overview](architecture.md),
and the [README](../README.md).

> Yeollin CMS is v0.1 and pre-release. Read the
> [Known limitations](#known-limitations) section before deciding where to run
> it.

## What you deploy

`cargo run -p yeollin-cli -- build` produces a single executable. The frontend
has already been statically exported and embedded with `include_dir!`, so the
binary serves both the API and the UI. There is no separate Node process to run
and no static file directory to ship alongside it.

## `JWT_SECRET`

The signing secret for access tokens. It has one hard requirement: at least 32
bytes.

`AuthConfig::validate` rejects anything shorter, and `YeollinApp::run` calls it
before serving traffic. A short or empty secret is a startup failure, not a
degraded mode, because the alternative is quietly issuing forgeable sessions. The
32-byte floor matches the HS256 output size, below which the key rather than the
signature becomes the weak link.

Generate one and keep it out of the repository:

```bash
export JWT_SECRET="$(openssl rand -base64 48)"
```

Deployment notes:

- A deployed binary is never given a secret automatically. `yeollin dev` mints an
  ephemeral secret for local sessions only; that path does not exist in
  production.
- Rotating the secret invalidates every outstanding access token.
- Keep it in your platform's secret store, not in a shell profile or an image
  layer.

Default token lifetimes are one hour for access tokens and seven days for refresh
tokens. Both are configurable on `AuthConfig` through `access_token_expiry` and
`refresh_token_expiry`.

## Credentials live in the `auth` plugin

The framework has no credential store. the `auth` plugin owns the `users`
and `sessions` tables and exposes the auth endpoints. Remove the plugin and there
is no way to sign in; the framework does not fall back to anything.

| Endpoint | Method | Reachable without an access token |
|----------|--------|-----------------------------------|
| `/api/auth/login` | POST | yes |
| `/api/auth/refresh` | POST | yes |
| `/api/auth/logout` | POST | yes |
| `/api/auth/me` | GET | no |

`logout` is deliberately reachable without a valid access token: it authenticates
with the refresh token it carries, so a client whose access token has already
expired can still revoke its session. `/health` is public as well.

### Password storage

Passwords are stored as Argon2 PHC hashes in `users.password_hash` and are never
compared in plaintext. Login looks the account up by trimmed, lowercased
username, then verifies the presented password against the stored hash.

A failed login returns the same `401 INVALID_CREDENTIALS` response whether the
username does not exist or the password is wrong, so the endpoint cannot be used
to enumerate valid usernames.

### Refresh tokens

Refresh tokens are deliberately not JWTs.

| Property | How |
|----------|-----|
| Opaque | 32 bytes of randomness, hex-encoded. They carry no claims. |
| Hashed at rest | Only the SHA-256 digest is stored, in `sessions.refresh_token_hash`. A database leak exposes no usable token. |
| Single-use | `/api/auth/refresh` revokes the presented token *before* minting the replacement, so replaying it can never yield a second valid pair. |
| Revocable | `/api/auth/logout` sets `revoked_at`. A self-contained signed token could not express revocation, which is the reason for the design. |
| Expiring | `sessions.expires_at` is checked on every refresh alongside `revoked_at`. |

SHA-256 rather than Argon2 is the right choice here: the input is 256 bits of
uniform randomness, so there is nothing to brute-force, and lookup needs a
deterministic digest.

`logout` always reports success, whether or not the presented token existed.
Whether a token was valid is not the caller's business.

## Bootstrapping the first administrator

`auth` seeds one administrator from the environment, and only while the
`users` table is empty:

| Variable | Purpose |
|----------|---------|
| `YEOLLIN_ADMIN_USERNAME` | Username. Trimmed and lowercased. |
| `YEOLLIN_ADMIN_PASSWORD` | Password, at least 12 characters. Hashed with Argon2 on insert. Startup fails with the reason if it is shorter, rather than seeding a weak administrator. |

Behaviour worth knowing:

- The seed runs only when the user count is zero. It cannot silently reset an
  existing account or resurrect a deleted one.
- If either variable is set to an empty value, startup fails rather than creating
  an account nobody intended.
- If no users exist and the variables are unset, the plugin logs a warning that
  nobody can sign in, and startup continues.
- The seeded account gets the role `admin`.

**Unset both variables after the first successful start.** Leaving a plaintext
password in the process environment, the unit file, or the container spec keeps
it readable by anything that can inspect the process, for no further benefit.
Sign in, change the password if your deployment process handed it around, then
remove the variables from the environment and restart.

There are no `SUPERADMIN_USERNAME` or `SUPERADMIN_PASSWORD` variables and no
framework-level superadmin account.

## Managing accounts afterwards

Administrators manage accounts from the **Users** page at `/auth`, or over the
API under `/api/auth/users`. Three refusals are deliberate and will look like
bugs if you do not expect them:

- **The only administrator cannot be demoted or deleted.** Promotion requires the
  administrator role, so there would be nobody left able to grant it back and
  recovery would mean editing the database by hand. Promote a second account
  first.
- **An account cannot delete the one it is signed in as**, even when other
  administrators exist, since the caller's own session would die mid-request.
- **An unrecognised role is refused rather than stored.** Role matching is exact,
  so a typo like `Admin` would be a role that grants nothing while looking
  deliberate. The accepted values are `admin` and `user`.

Changing or resetting a password deletes every session for that account, so the
holder of a stolen refresh token loses access at the moment the password changes.
The person who changed it signs in again.

## Route access is deny-by-default

Page routes are discovered from the App Router directory tree, but nothing
security-relevant is inferred from directory *names*.

- Every route is `authenticated` unless a `route.meta.json` sidecar beside its
  `page.tsx` says otherwise.
- `access` accepts `authenticated` (the default), `public`, or `guest`.
- Route groups such as `(public)` and `(guest)` organise files and **grant
  nothing**. A page under `(public)` without `"access": "public"` still requires
  a session.
- `menu` controls navigation only, never authorization. Hiding a page from the
  menu does not protect it.
- Unknown fields, invalid values, duplicate route paths, and `menu: true` on a
  dynamic route all fail the build. There is no silent fallback to a default.

At runtime, unauthenticated requests to `/api/*` get `401 UNAUTHORIZED` as JSON;
everything else is redirected to the sign-in page. An authenticated request to a
guest route gets `403 ALREADY_AUTHENTICATED` for `/api/*`, or a redirect to the
dashboard otherwise.

Public and guest route matching compares whole paths, after collapsing trailing
slashes. `/health` matches, `/healthz`, `/health-check`, and `/health/details` do
not. A prefix can never widen access. Any path containing `..` is refused
outright rather than normalised, so a traversal attempt cannot resolve into a
route that was declared public.

## Dev-only asset paths

The auth middleware exempts frontend assets from authentication. Two groups
behave differently:

| Group | Paths | Exempt when |
|-------|-------|-------------|
| Built output | `/_next/`, `/static/`, and the exact file `/favicon.ico` | always |
| Vite dev server | `/@`, `/__vite_hmr`, `/node_modules/`, `/src/`, `/df/` | only while `dev_mode` is on |

`dev_mode` is off by default and is turned on only when the dev proxy is active,
which is gated by `YEOLLIN_DEV_PROXY`. Those dev paths do not exist in a release
binary, so exempting them in production would only widen the bypass surface for
routes the application itself might still answer.

Do not enable the dev proxy in production.

Exempt files are matched exactly, never by suffix. Matching `.ico` as a suffix
would have let `/api/example-memo-plugin/1.ico` reach a handler unauthenticated; the tests in
`crates/auth/src/middleware.rs` lock that behaviour down, along with the
traversal and prefix cases above.

## Run behind TLS

The binary speaks plain HTTP. Terminate TLS in front of it with a reverse proxy
such as nginx or Caddy, or with your platform's load balancer, and do not expose
the application port directly.

This matters more than usual here because tokens travel in cookies. The sign-in
page sets `access_token` and `refresh_token` as cookies on the browser; without
TLS they cross the network in the clear.

Bind the application to an address your proxy can reach and nothing else.
`apps/example-app` binds `0.0.0.0` and reads its port from `PORT`, defaulting to
3001; adjust the host for your own application if it should only accept traffic
from the proxy.

## Back up the SQLite file

`apps/example-app` stores everything in one SQLite file:
`sqlite://./db.sqlite?mode=rwc`. `mode=rwc` creates it on first run, relative to
the process working directory. Vespertide provisions the schema during plugin
initialisation.

That file holds your users, your Argon2 password hashes, your active sessions,
typed plugin settings, the transactional event outbox, and all plugin data.
Treat it as the whole application state:

- Back it up on a schedule, and test a restore.
- Copy it with a SQLite-aware method rather than a naive file copy of a live
  database, so you do not capture a torn write.
- Store backups encrypted. They contain password hashes and refresh-token
  hashes.
- Pin the working directory in your service definition. A relative path means a
  different launch directory silently creates a second, empty database.

Committed but unprocessed event rows are recovery state, not disposable logs.
Do not delete them during a deployment. The Deferred drainer polls after startup
and may deliver a row more than once if the previous process stopped after the
subscriber succeeded but before it marked the row processed; Deferred consumers
must therefore be idempotent.

The `audit-log` plugin exposes only events whose type explicitly sets
`Event::AUDIT = true`, and its API requires the exact `admin` role. Treat those
JSON payloads as sensitive operational data: event producers must omit secrets
and minimise personal data. Retention defaults to 90 days and is configurable
through `/audit-log/settings` from 1 through 3650 days. The retention pass runs
at startup, after event delivery, and before reads, but removes only processed
audit rows; pending outbox work and non-audit events are preserved.

## Known limitations

Yeollin CMS is v0.1 and pre-release. The following are known gaps, not oversights
in this document.

- **Pre-release.** Every public interface, file layout, CLI flag, and API field
  is unstable and changes without notice. There is no upgrade path guarantee
  between versions.
- **Login throttling is per-process and in-memory.** `/api/auth/login` allows 5
  failed attempts per username and source address within 5 minutes, then answers
  `429 TOO_MANY_ATTEMPTS` until the window elapses. The counters live in the
  process, so they reset on restart and are not shared between instances. Run a
  single instance, or add rate limiting in the reverse proxy as well. There is no
  backoff and no CAPTCHA.
- **Authorization is opt-in per handler.** The middleware authenticates; it does
  not decide what a caller may do, so every signed-in account reaches every
  protected route until a handler says otherwise. `Authorize` on `CurrentUser`
  provides `require_role`, `require_any_role`, and `has_role`, but a plugin that
  forgets to call one leaves its endpoint open to any authenticated user. Audit
  administrative and destructive endpoints for a role check.
- **A capability check is not a sandbox.** Plugins are statically linked, so
  third-party plugin code runs with full process privileges. `require_role`
  enforces *user* authorization; it does not isolate the plugin itself. Only
  install plugins you trust as much as the application.
- **No browser test coverage.** CI runs clippy, `cargo test`, `cargo doc`,
  oxlint, `tsc --noEmit`, and a frontend build. Four of those tests boot the
  real binary and drive it over HTTP, but nothing exercises the application
  through a browser.

## Deployment checklist

1. Build the release binary with `cargo run -p yeollin-cli -- build`.
2. Set `JWT_SECRET` to at least 32 bytes from a secret store.
3. Set `PORT`, and a working directory that pins the SQLite file location.
4. Confirm `YEOLLIN_DEV_PROXY` is **not** set.
5. Start once with `YEOLLIN_ADMIN_USERNAME` and `YEOLLIN_ADMIN_PASSWORD` to seed
   the first administrator.
6. Sign in, verify the account, then unset both bootstrap variables and restart.
7. Put a TLS-terminating reverse proxy in front. Add rate limiting on
   `/api/auth/login` there too if you run more than one instance, since the
   built-in throttle is per-process.
8. Schedule and verify backups of the SQLite file.
9. Review every `route.meta.json` with `"access": "public"` or `"guest"` and
   confirm each one is deliberate.
