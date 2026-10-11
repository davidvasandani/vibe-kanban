# Implementation Plan: Session chat history through the VK MCP

**Spec**: `./spec.md`
**Status**: Planned

## Technical Context

- Rust backend (`axum`, `sqlx`/SQLite, `chrono`), MCP server in `crates/mcp`
  (`rmcp` `#[tool_router]`), which talks HTTP to the backend.
- Message source: `ContainerService::normalized_entries`
  (`crates/services/src/services/container.rs`), which already serves
  `/messages`. It replays the normalized sidecar for settled turns, snapshots
  the live `MsgStore` for running ones, and falls back to bounded legacy
  re-normalization behind the global historical permit.
- Turn list: `ExecutionProcess::find_by_session_id(pool, id, false)`
  (`crates/db/src/models/execution_process.rs`), ordered by `created_at ASC`.
- No new dependencies and no DB migration.

## Architecture & Approach

### Backend: shared projection (`crates/server/src/routes/execution_processes.rs`)

- Split `project_messages` into a reusable per-entry projector,
  `project_entry(index, entry, execution_id) -> Option<(SessionMessage, &str)>`,
  returning the message plus the untruncated text. `project_messages` keeps
  its signature and behaviour (FR-12), and its existing tests stay green.
- Add `match_snippet(text, needle_lower) -> Option<String>`. It is char-safe,
  gives about 80 chars on each side, and marks clipped sides with `…`.
  Matching is done on lowercase text with char-index mapping, so non-ASCII
  text cannot split a code point.

### Backend: history route (`crates/server/src/routes/sessions/history.rs`, new module)

Kept out of `sessions/mod.rs` so the pure helpers and their tests sit
together, the way `sessions/review.rs` is split out.

- `SessionHistoryQuery` (serde `Deserialize`): `since`, `until`, `query`,
  `roles`, `execution_id`, `limit`, `offset`, `order`.
- Pure helpers, all unit-tested:
  - `parse_bound(&str) -> Result<DateTime<Utc>, ApiError>` (RFC 3339), and
    `since > until` → `ApiError::BadRequest`.
  - `turn_overlaps(started, completed_or_now, since, until) -> bool` (FR-8).
  - `effective_time(entry_ts, turn_started)` (FR-7).
  - `HistoryFilter::matches(msg, untruncated, effective_time)`, covering the
    window, keyword and roles (roles are already applied by the projector).
  - `paginate(Vec<T>, order, offset, limit) -> (Vec<T>, total, has_more)` (FR-6).
- Handler `get_session_history`:
  1. Load coding-agent turns (non-dropped). Record `turn_index` (0-based
     position among the session's coding-agent turns).
  2. With `execution_id`: keep that turn, else `ApiError::BadRequest`
     ("execution is not a coding-agent turn of this session") (FR-5, FR-11).
  3. Prune by `turn_overlaps` (FR-8).
  4. Loop sequentially. Before each turn, if `Instant::now() >= deadline`
     (20 s), push the turn and all remaining ones to `unscanned_execution_ids`
     and stop (FR-9). Otherwise `normalized_entries(&id).await`, project,
     filter and collect.
  5. Paginate and respond.
- Registered as `.route("/history", get(history::get_session_history))` in
  `sessions::router`, so it gets `load_session_middleware` (404 for an unknown
  session).

### MCP tool (`crates/mcp/src/task_server/tools/sessions.rs`)

- `SearchSessionMessagesRequest` with schemars descriptions. `session_id` is
  required; the rest are optional.
- `search_session_messages`: GET `/api/sessions/{id}` → `scope_allows_workspace`
  (FR-10) → GET `/api/sessions/{id}/history` with the query, deserialized into
  a mirrored `SessionHistoryPayload` and returned as-is via `Self::success`.
- `tools/mod.rs`: add the name to the orchestrator-mode expected tool set test.
- Add an earlier-turns pointer to the `list_recent_messages` and
  `list_all_messages` descriptions (FR-12).

### Docs

- `crates/mcp/AGENTS.md` ("Reading a session before following up"): a bullet
  for the new tool, its filters, the budget and the timestamp fallback.

## Data Model

See `./data-model.md`. No persisted data changes. Response types only.

## Contracts

See `./contracts/session-history.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- **III / VI (small steps, don't rebuild)**: reuses `normalized_entries`
  and the `/messages` projection rules. No new store or socket.
- **II (test the contract)**: pure helpers carry `#[cfg(test)]` coverage, and
  the MCP tool-set test is updated.
- **XXXI (materialized once)**: per-turn reads go through the existing
  single-flight sidecar path. Sequential loading avoids fanning out across
  the global permit.
- **XXXIX (request reads terminate)**: `normalized_entries` already
  snapshots running turns rather than tailing them.
- **XLIII / LII (budget)**: a 20 s budget checked between turns. A miss is
  reported as `complete: false` with `unscanned_execution_ids` and starts no
  extra work.
- **XX (event provenance)**: `created_at` keeps the entry's own value (null if
  absent). The fallback `turn_started_at` is explicit, and "now" is never
  fabricated.
- **XLIV**: finite read over HTTP. No WebSocket.
- Generated types: the new structs derive `TS` for parity with
  `RecentMessagesResponse` but are not exported in `generate_types.rs`, so
  `shared/types.ts` is unchanged.

No deviations.

## Risks & Dependencies

- **Large sessions with many cold legacy turns** can exceed the budget. This
  is mitigated by pruning, the `complete` flag and sidecar warm-up on retry.
- **Text cost**: up to 200 × 4000 chars ≈ 800 KB worst case per page. This is
  acceptable for MCP clients and matches the existing 100-message reader's order
  of magnitude.
- **Memory**: all matches of the scanned turns are held before paging. That is
  bounded by the per-turn normalized projection the existing `list_all_messages`
  already materializes, times the number of turns. Only the page is cloned
  out, with truncated text.
