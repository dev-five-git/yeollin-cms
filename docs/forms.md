# Forms plugin

`forms` supplies one administrator screen at `/forms`, a public form-definition
endpoint, and a public submission endpoint. It is registered in
`apps/example-app`; another application registers it in the usual way:

```bash
cd apps/my-app
yeollin plugin add forms
yeollin plugin doctor
```

## Administrator workflow

Administrators create forms in the **Forms** screen. A form has a display name,
description, success message, enabled switch, hourly submission limit, and 1–20
fields. Supported field types are short text, email, long text, checkbox, and
select. Field IDs are stable lowercase kebab-case names; changing an ID creates
a different value in future submissions.

The submission inbox stores the exact field definition that accepted every
submission. Editing a form therefore never changes the labels or interpretation
of a historic response. Deleting a form deliberately deletes its submission
inbox in the same transaction.

All administrator routes require the exact `admin` role:

| Method | Path | Purpose |
|---|---|---|
| `GET` / `POST` | `/api/forms` | List or create forms |
| `GET` / `PUT` / `DELETE` | `/api/forms/{id}` | Read, replace, or remove a form |
| `GET` | `/api/forms/{id}/submissions` | Read its paginated submission inbox |

## Public integration

Only the following two exact paths are public. A form ID is a 32-character,
lowercase hexadecimal opaque identifier, not a name or a filesystem path.

```text
GET  /api/forms/public?id=<form-id>
POST /api/forms/submit
```

The public definition contains only the fields needed to render the form:
`id`, `name`, `description`, `fields`, and `successMessage`. It never exposes
the creator or the submission quota. A client submits JSON in this shape:

```json
{
  "formId": "0123456789abcdef0123456789abcdef",
  "values": {
    "email": "visitor@example.com",
    "terms": true
  }
}
```

The server rejects unknown field IDs, wrong JSON types, blank required values,
invalid email values, overlong text, and select values not declared by the
form. It trims text values before saving. The per-form hourly cap is a hard
global ceiling intended to keep a public endpoint bounded; a saturated form
returns `429`. For visitor-specific abuse controls, put a rate-limiting proxy
or CAPTCHA service in front of the public endpoint.

## Privacy and events

Form definitions, form configuration changes, and inbox reads are
administrator-only. A successful public submission stores its values only in
the form submission table. It emits `forms.submitted` transactionally with the
form ID and submission ID but **never values**; the event is not audit-marked.
This keeps PII out of `audit-log` and prevents generic webhook subscriptions
from forwarding submission content by mistake.
