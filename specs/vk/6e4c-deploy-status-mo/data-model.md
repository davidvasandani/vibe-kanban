# Data Model: Mobile Deploy Status

## Deployment metadata

| Field | Type | Source | Semantics |
| --- | --- | --- | --- |
| `version` | string | build-stamped `VK_GIT_SHA` | Short Git SHA for the running server; `dev` when unstamped. Existing field. |
| `deployment_timestamp` | optional RFC 3339 UTC string | immutable release build | Stable creation instant shared by the running binary and `release.json`. New additive field. |

There is no persistence or database migration. A release build creates one
timestamp; every `/api/info` response from that build returns the same value,
and development builds may return no value.

## Derived mobile view model

- `revisionLabel`: the available `version`, displayed compactly.
- `revisionUrl`: commit URL only when the revision is present and not `dev`.
- `ageLabel`: `now`, completed minutes, completed hours, or completed days
  derived from `deployment_timestamp` and current browser time.

The status is omitted only when neither a revision nor a valid timestamp is
available.
