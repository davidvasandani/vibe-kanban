# Session chat history search (MCP)

`search_session_messages` (`crates/mcp/src/task_server/tools/sessions.rs`)
lets an MCP caller read any part of a session's chat log. It calls
`GET /api/sessions/{id}/history`
(`crates/server/src/routes/sessions/history.rs`). The older readers
(`list_recent_messages`, `list_all_messages`, `get_execution.final_message`)
are per-execution: with only a `session_id` they resolve the session's
**latest** coding-agent turn, and the MCP had no way to list earlier turns.

## Shape

- A session's turns are its non-dropped `CodingAgent` execution processes, in
  `created_at` order. `turn_index` is the position among those turns. Dropped
  turns are excluded so results match the visible chat.
- Messages come from the same projection as `/messages`. `project_entry` in
  `routes/execution_processes.rs` holds the role mapping, the empty-text skip,
  the 4000-char truncation and the `{execution_id}:{index}` ids. It also
  returns the **untruncated** text, so keyword filters can see past the cap.
  Reuse it for any new reader rather than re-deriving roles.
- Order is (turn, entry index), not effective timestamp. Sorting by a
  timestamp that falls back for most entries would be unstable.
- `limit`/`offset` paging, with `total_matches`/`has_more`. Offset is safe
  because settled turns are immutable and a running turn only grows at the
  end.

## Timestamps are mostly per turn

`NormalizedEntry.timestamp` exists, but the normalizers rarely set it. The
effective time is the entry's RFC 3339 timestamp, else the turn's
`started_at`. The response keeps `created_at` (the entry's own value, often
null) separate from `turn_started_at`, so the fallback is visible and no time
is invented. In practice `since`/`until` select whole turns. If a normalizer
starts emitting timestamps, the filter becomes per message with no route
change.

## Bounding the cost

- **Prune before reading.** A turn whose `[started_at, completed_at or now]`
  span misses the window is dropped using execution metadata alone, so its
  log is never loaded.
- **Load sequentially.** A legacy turn without a normalized sidecar competes
  for the global historical-normalization permit (see
  `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`), so
  parallel loads would only queue and starve the UI's own history reads.
- **Budget between turns (20 s).** Once the budget is spent, no new turn
  starts. The one in flight finishes, so its sidecar is written and a retry
  is cheaper. Skipped turns go in `unscanned_execution_ids` and set
  `complete: false`.
- `scan_turns` is generic over the loader and the budget check, so tests can
  drive a budget miss without a DB or a clock.

## Gotcha: `None` is not "empty"

`ContainerService::normalized_entries` returns `Option`. `None` means the
log could not be read or materialized (or no log exists). The first version
did `unwrap_or_default()`, which counted that turn as **searched with zero
matches** and could report `complete: true` while history was missing. Codex
review caught this. Such turns now go in `unreadable_execution_ids` and also
set `complete: false`. They are separate from budget skips because retrying
will not fix them. Any reader that aggregates over several executions should
keep "could not read" distinct from "read, nothing matched".

## Relation to global search

`routes/global_search.rs` also finds old turns, but only through
`coding_agent_turns.prompt`/`.summary` (the user prompt and final reply),
with a 20-snippet cap, for the UI dialog. It cannot see intermediate
commentary or tool output and has no MCP surface. The two complement each
other. Their snippet helpers differ on purpose: global search's fixed
240-char window feeds that UI unchanged.

## Contributed by

- vk/5b8a-vk-mcp-chat-logs
