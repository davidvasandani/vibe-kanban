# Data model: discoverable mobile workspace right drawer

No backend or persisted data model changes are required.

The in-memory mobile tab descriptor gains one optional presentation field:

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `MobileTabId` | Stable routing/preference key; remains `git` |
| `icon` | `Icon` | Presentational glyph; right-sidebar metaphor |
| `label` | `string` | Visible wider-mobile label; `Sidebar` |
| `accessibleLabel` | `string?` | Explicit control name; `Right sidebar` |

The existing persisted `MobileTab` union and value are unchanged.

