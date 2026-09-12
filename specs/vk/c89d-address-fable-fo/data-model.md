# Data Model

## PreviewSettingsData

Existing per-workspace scratch payload, expanded compatibly:

| Field | Type | Meaning |
|---|---|---|
| `url` | string | Existing complete manual override URL; empty means no override. |
| `current_route` | optional string | Latest auto-detected app route as pathname, query, and fragment, beginning with `/`. |
| `screen_size` | optional string | Existing desktop/mobile/responsive selection. |
| `responsive_width` | optional integer | Existing responsive viewport width. |
| `responsive_height` | optional integer | Existing responsive viewport height. |

`current_route` is optional and defaults to absent so all existing serialized
scratch payloads remain valid. It never contains an origin, credentials, or
Vibe Kanban transport-only query parameters.

## State transitions

- Accepted auto-detected navigation: extract canonical route and update
  `current_route` if changed.
- Preview remount/reload: combine `current_route` with the freshly auto-detected
  origin and port.
- Manual override set: update `url`; do not overwrite `current_route`.
- Manual override navigation: do not update `current_route`.
- Manual override clear: set `url` to empty; retain `current_route` and viewport
  settings.
- Workspace change: load that workspace's independent scratch payload.
