# Data Model: Claude Executor Catalog Refresh

No persisted data model or generated API schema changes.

The in-memory fallback catalog adds one `ModelInfo` value:

| Field | Value |
| --- | --- |
| `id` | `claude-fable-5-1` |
| `name` | `Fable 5.1` |
| `provider_id` | `None` |
| `reasoning_options` | `low`, `medium`, `high`, `xhigh`, `max` |

Existing executor configuration already stores a selected model as an opaque
string and passes it through to Claude Code, so no migration is needed.
