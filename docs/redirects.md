# Redirects plugin

`redirects` provides the administrator screen at `/redirects` for replacing
legacy page URLs without changing application routes. Register it from an app
directory in the usual way:

```bash
cd apps/my-app
yeollin plugin add redirects
yeollin plugin doctor
```

## Behavior

Each enabled rule has one exact, canonical source path and one destination.
The application evaluates redirects for `GET` and `HEAD` requests before
authentication and static-file fallback, and returns HTTP `308 Permanent
Redirect`. This lets a legacy page URL continue to work even when that page is
not publicly accessible. Query strings do not affect matching and are not
carried to the destination.

Sources must be root-relative paths such as `/old-pricing`; they cannot include
query strings, fragments, `..`, duplicate or trailing slashes, whitespace, or
backslashes. The site root, API routes, health checks, Vite/framework assets,
and the favicon are reserved and cannot be sources. Destinations are either a
canonical internal path or an `https://` URL with a host.

Disabling a rule preserves it for later reuse and falls through to the normal
application response. Rules are exact matches: `/old-pricing` does not match
`/old-pricing/archive`.

## API

All endpoints require an authenticated administrator.

| Method | Path | Purpose |
|---|---|---|
| `GET` / `POST` | `/api/redirects` | List or create rules |
| `GET` / `PUT` / `DELETE` | `/api/redirects/{id}` | Read, replace, or remove a rule |

Request fields use camel case: `sourcePath`, `destinationPath`, and `enabled`.
Identifiers are opaque lowercase hexadecimal strings. Every create, update,
and deletion emits an audit-marked redirect event with configuration metadata;
there is no public redirect-management API.
