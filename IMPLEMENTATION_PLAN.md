# Implementation plan: VK MCP session chat history

Task `vk/5b8a-vk-mcp-chat-logs`. See `SPEC.md`.

## Step 1: Backend pure logic (`crates/server/src/routes/execution_processes.rs`)

1. Factor the per-entry projection out of `project_messages` into
   `project_entries(entries, execution_id) -> Vec<(SessionMessage, untruncated
   &str)>` (or an equivalent iterator), so `/messages` and the new history
   route share role mapping, the empty-text skip, truncation and ids.
   `project_messages` keeps its signature and behaviour.
2. Add `match_snippet(text, needle_lower) -> Option<String>`, which returns a
   char-safe window of about 160 chars around the first case-insensitive hit,
   with `…` on clipped sides.
3. Add `entry_effective_time(entry_ts: Option<&str>, turn_started_at) ->
   DateTime<Utc>`, which parses RFC 3339 and falls back to the turn start.

## Step 2: Backend route (`crates/server/src/routes/sessions/mod.rs`)

1. `SessionHistoryQuery { since, until, query, roles, execution_id, limit,
   offset, order }` (serde, with `since`/`until` as `Option<String>` parsed in
   the handler so the error message is useful).
2. `SessionHistoryResponse` / `SessionHistoryMessage` (Serialize + TS).
3. `turn_overlaps(started, completed, since, until) -> bool`, which is pure.
4. A `paginate(matches, order, offset, limit)` helper, which is pure.
5. `get_session_history` handler: load turns, validate `execution_id`,
   prune, load entries sequentially, filter, paginate and respond. Register
   `.route("/history", get(get_session_history))`.
6. Unit tests: overlap, effective time fallback, snippet (unicode, edges,
   no hit), paging (`asc`/`desc`, offset past end, `has_more`), keyword
   matching beyond the truncation cap, role filter.

## Step 3: MCP tool (`crates/mcp/src/task_server/tools/sessions.rs`, `tools/mod.rs`)

1. `SearchSessionMessagesRequest` (schemars descriptions) and a mirrored
   `SessionHistoryPayload` / response.
2. `search_session_messages` tool: fetch the session, check scope, GET
   `/api/sessions/{id}/history` with the query, then return.
3. Register the name in the tool list in `tools/mod.rs`.
4. Add a hint about earlier turns to the `list_recent_messages` and
   `list_all_messages` descriptions.

## Step 4: Docs

- `crates/mcp/AGENTS.md`: add a `search_session_messages` bullet under
  "Reading a session before following up".

## Step 5: Verify

- `cargo test -p server` (route unit tests), `cargo test -p mcp`.
- `cargo clippy` on the touched crates, `pnpm run format`.
- `pnpm run generate-types:check`, only if TS-derived types get exported
  (the new types are not added to `generate_types.rs`, matching
  `RecentMessagesResponse`).
