# Security Policy

## Project status

Yeollin CMS is pre-release software at v0.1. It has not been through a security
audit, and its interfaces change without notice. **It is not yet suitable for
handling production data.** Use it for development, evaluation, and
experimentation only, with data you can afford to lose or disclose.

Credentials live in the `auth-users` plugin. It creates the first administrator
from `YEOLLIN_ADMIN_USERNAME` and `YEOLLIN_ADMIN_PASSWORD` only while the users
table is empty, stores the password as an Argon2 hash, and never compares a
password in plaintext. The server refuses to start unless `JWT_SECRET` is at
least 32 bytes. Once the first administrator exists, unset the two bootstrap
variables so no plaintext password remains in the environment.

## Supported versions

Only the `main` branch is supported. Fixes land on `main`; there are no
maintained release branches and no backports.

| Branch | Supported |
|--------|-----------|
| `main` | Yes |
| Anything else | No |

## Reporting a vulnerability

Report privately. Do not open a public issue and do not describe the problem in
a pull request.

Open a private security advisory on the
[`dev-five-git/yeollin-cms`](https://github.com/dev-five-git/yeollin-cms)
repository, under the Security tab, using "Report a vulnerability".

Please include:

- What the issue is and which component is affected.
- The commit or branch you observed it on.
- Steps to reproduce, ideally minimal.
- What an attacker could achieve with it.

Keep the details in the advisory until a fix is on `main`.
