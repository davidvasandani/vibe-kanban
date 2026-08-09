# Data Model: Mobile Deploy Status

## Deployment metadata

| Field | Type | Source | Rules |
| --- | --- | --- | --- |
| `version` | string | Embedded `VK_GIT_SHA` | Existing short SHA; `dev` sentinel when unstamped. |
| `deployment_timestamp` | optional string | Embedded `VK_BUILD_TIMESTAMP` | UTC ISO-8601 timestamp shared with `release.json.built_at`; absent for unstamped/legacy builds. |

## Derived presentation

| Value | Derivation | Rules |
| --- | --- | --- |
| Short revision | `version` | Display as supplied; production values link to exact GitHub commit; `dev` does not. |
| Elapsed age | `now - deployment_timestamp` | Clamp future/negative ages to the newest bucket; invalid/missing values render no age. Use compact units appropriate to age. |
| Accessible description | revision + expanded elapsed meaning | Must identify deployment context without depending on punctuation or compact unit knowledge. |

No value is persisted in application storage. The server response is authoritative for the running binary, and elapsed age is derived client-side.

