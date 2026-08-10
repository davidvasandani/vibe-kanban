# Data Model: Mobile Deploy Status

## Deployment metadata

| Field | Type | Source | Semantics |
| --- | --- | --- | --- |
| `version` | string | build-stamped `VK_GIT_SHA` | Short Git SHA for the running server; `dev` when unstamped. Existing field. |
| `started_at` | RFC 3339 UTC string | server process/router initialization | Stable start instant of the currently running deployment process. New additive field. |

There is no persistence or database migration. A service restart creates a new
`started_at`; every `/api/info` response from that process returns the same
value.

## Derived mobile view model

- `revisionLabel`: the available `version`, displayed compactly.
- `revisionUrl`: commit URL only when the revision is present and not `dev`.
- `ageLabel`: `now`, completed minutes, completed hours, or completed days
  derived from `started_at` and current browser time.

The status is omitted only when neither a revision nor a valid timestamp is
available.
