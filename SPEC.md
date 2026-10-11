# SPEC: VK MCP session chat history (filterable, every turn)

Task: `vk/5b8a-vk-mcp-chat-logs`. Feature spec, plan and tasks:
`specs/vk/5b8a-vk-mcp-chat-logs/`.

## Problem

The VK MCP message readers (`list_recent_messages`, `list_all_messages`) read
**one execution**: the given `execution_id`, or the session's *latest*
coding-agent turn when only `session_id` is passed. A session is a chain of
turns (one `CodingAgent` execution process per prompt). Earlier turns can be
read only if the caller already knows each execution id, and the MCP exposes no
way to list them. Nothing lets a caller ask "what was said in this session
yesterday afternoon" or "where did the agent mention `migration`".

## Goal

Any part of a session's chat log can be retrieved through the VK MCP, across
every coding-agent turn, with filters:

- **Date/time window**: `since` / `until` (RFC 3339).
- **Keyword**: case-insensitive substring match on message text.
- **Role**: the existing `user,assistant,system,tool` filter.
- **Turn**: restrict to one `execution_id`.
- **Paging**: `limit` + `offset` over the filtered, chronologically ordered
  result, with `total_matches` and `has_more` so the caller can walk all of it.

## Design

### Backend: `GET /api/sessions/{id}/history`

New route in `crates/server/src/routes/sessions/mod.rs`, next to
`/messages`. Query parameters:

| param | type | meaning |
|---|---|---|
| `since`, `until` | RFC 3339 | inclusive time window on the message timestamp |
| `query` | string | case-insensitive substring filter (trimmed; empty = none) |
| `roles` | csv | same parser as `/messages` (`parse_roles`) |
| `execution_id` | uuid | only that turn (must belong to the session) |
| `limit` | int | default 50, clamped to 1..=200 |
| `offset` | int | skip this many matches (default 0) |
| `order` | `asc`/`desc` | chronological (default `asc`); `desc` returns newest first |

Algorithm:

1. `ExecutionProcess::find_by_session_id(pool, id, false)` → keep
   `run_reason == CodingAgent`, ascending by `created_at` (already the order).
2. If `execution_id` is set, keep only it (404 if it is not one of the
   session's coding-agent turns).
3. **Prune turns by time window** before reading any logs: a turn whose
   `[started_at, completed_at or now]` interval does not overlap
   `[since, until]` cannot contain a match, so its log is never loaded.
4. For each remaining turn, `ContainerService::normalized_entries` (the same
   source `/messages` uses — no new store) and project entries with the
   existing `project_messages` logic (role mapping, empty-text skip,
   4000-char truncation, `{execution_id}:{index}` ids).
5. Each message's **effective timestamp** is its entry `timestamp` when the
   executor reported a parseable one, otherwise the turn's `started_at`. This
   is exposed as `created_at` (unchanged meaning) plus `turn_started_at`; a
   message falls in the window based on its effective timestamp. Most
   normalizers leave entry timestamps unset, so in practice time filtering
   is turn-granular; that is documented in the tool description.
   Turns are loaded **sequentially**, because legacy turns without a
   normalized sidecar compete for the global historical-normalization permit.
6. Keyword filter runs on the **untruncated** text, so a match past the
   4000-char cap is still found; a `match_snippet` (≈ 160 chars of context
   around the first hit) is returned so the match is visible even when the
   returned `text` is truncated.
7. **Budget** (constitution LII / XLIII): before loading each turn, check
   a 20 s request budget. Once it has passed, stop loading further turns
   (the turn in progress finishes, so its sidecar is still written for the
   next read). The response sets `complete: false` and lists
   `unscanned_execution_ids`, so the caller can retry or narrow the window.
8. Order, then apply `offset`/`limit`; report `total_matches`, `has_more`
   (`offset + returned < total_matches`), `turns_scanned`, and
   `turn_count` (all coding-agent turns in the session).

Response (`SessionHistoryResponse`):

```jsonc
{
  "session_id": "…",
  "turn_count": 12,
  "turns_scanned": 3,
  "total_matches": 41,
  "offset": 0,
  "limit": 50,
  "has_more": false,
  "complete": true,
  "unscanned_execution_ids": [],
  "unreadable_execution_ids": [],
  "messages": [
    { "id": "<exec>:<idx>", "role": "assistant", "text": "…",
      "created_at": "…|null", "turn_started_at": "…",
      "execution_id": "…", "turn_index": 4, "match_snippet": "…|null" }
  ]
}
```

Invalid `since`/`until` → 400 with a clear message. `since > until` → 400.

### MCP: `search_session_messages`

New tool in `crates/mcp/src/task_server/tools/sessions.rs`. Parameters mirror
the backend (`session_id` required, all filters optional). Scope check is the
same as the other readers: fetch the session, `scope_allows_workspace`. The
tool returns the backend response verbatim (typed deserialization like
`RecentMessagesPayload`). It is registered in the tool list in
`tools/mod.rs` next to `list_all_messages`.

Existing tools are **unchanged**. Their descriptions gain one sentence
pointing to `search_session_messages` for earlier turns, since "latest turn
only" is the exact gap this task closes.

### Why not extend `/messages`?

`/messages` and its two tools are a per-execution contract (status,
exit_code, final_message describe *one* execution). A session-wide history has
different response fields (paging, turn metadata) and different cost; mixing
them would make every existing field ambiguous.

## Acceptance criteria

1. With a session of ≥2 coding-agent turns, `search_session_messages` with no
   filters returns messages from **all** turns, oldest first, each tagged with
   its `execution_id` and `turn_index`.
2. `query="foo"` returns only messages whose text contains `foo`
   case-insensitively, including matches beyond the 4000-char truncation,
   each with a `match_snippet` containing the hit.
3. `since`/`until` excludes messages outside the window; turns entirely
   outside the window are not loaded (`turns_scanned` < `turn_count`).
4. Messages without an executor timestamp are filtered by their turn's
   `started_at`.
5. `limit`/`offset` page through results; `total_matches` and `has_more` are
   correct; `order=desc` returns newest first.
6. `execution_id` not in the session → error; malformed or inverted time
   bounds → 400.
7. If the budget runs out, the response says `complete: false` and names the
   unscanned turns. It never presents a partial scan as complete.
8. Out-of-scope sessions are rejected by the MCP like the other readers.
9. Unit tests cover the pure filter/paging/snippet/time-window logic.
10. `crates/mcp/AGENTS.md` documents the new tool.

## Out of scope

- Workspace-wide or cross-session search (callers can `list_sessions` and
  query each session).
- Regex or fuzzy matching.
- Returning text beyond the existing 4000-char per-message cap.
- Any UI change.
