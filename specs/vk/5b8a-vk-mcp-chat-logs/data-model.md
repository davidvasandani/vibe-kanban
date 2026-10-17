# Data model: session chat history

No persisted schema changes. Response-only types, in
`crates/server/src/routes/sessions/history.rs`, mirrored in
`crates/mcp/src/task_server/tools/sessions.rs`.

## SessionHistoryMessage

| field | type | notes |
|---|---|---|
| `id` | string | `{execution_id}:{entry_index}`, same as `/messages` |
| `role` | string | `user` \| `assistant` \| `system` \| `tool` |
| `text` | string | truncated to 4000 chars + `… [truncated]` |
| `created_at` | string? | the entry's own timestamp, as reported (null if absent) |
| `turn_started_at` | string | RFC 3339 `started_at` of the owning turn |
| `execution_id` | string | owning turn |
| `turn_index` | number | 0-based position among the session's non-dropped coding-agent turns |
| `match_snippet` | string? | present when `query` was given |

## SessionHistoryResponse

| field | type | notes |
|---|---|---|
| `session_id` | string | |
| `turn_count` | number | non-dropped coding-agent turns in the session |
| `turns_scanned` | number | turns whose log was actually read |
| `total_matches` | number | matches across scanned turns, before paging |
| `offset`, `limit` | number | effective (clamped) values |
| `order` | `asc` \| `desc` | |
| `has_more` | bool | `offset + messages.len() < total_matches` |
| `complete` | bool | false when any candidate turn went unsearched (budget or unreadable) |
| `unscanned_execution_ids` | string[] | candidate turns not read due to the budget |
| `unreadable_execution_ids` | string[] | candidate turns whose log could not be read (`normalized_entries` returned none); also makes `complete` false |
| `messages` | SessionHistoryMessage[] | |

## Ordering

Ascending = (turn order, entry index), which is chronological because turns
are ordered by `created_at` and entries by index. `desc` reverses this.
Ordering does **not** sort by effective timestamp, because a missing entry
timestamp would otherwise make the order unstable (constitution XXXIV).
