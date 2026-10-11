# Tasks: Session chat history through the VK MCP

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Shared projection
- [x] T001 Extract the per-entry projector `project_entry` (message + untruncated text) from `project_messages`, keeping `project_messages` behaviour identical; in `crates/server/src/routes/execution_processes.rs`
- [x] T002 Add `match_snippet` (case-insensitive, char-safe, ~80 chars each side, `…` on clipped sides) with unit tests in `crates/server/src/routes/execution_processes.rs` (after T001; same file)

## Phase 2: Backend route
- [x] T003 Create `crates/server/src/routes/sessions/history.rs`: `SessionHistoryQuery`, `HistoryOrder`, `SessionHistoryMessage`, `SessionHistoryResponse` (depends on T001)
- [x] T004 Add pure helpers `parse_bounds`, `turn_overlaps`, `effective_time`, `paginate`, `clamp_history_limit` in `crates/server/src/routes/sessions/history.rs` (depends on T003)
- [x] T005 Implement `get_session_history` handler (load turns, `execution_id` validation, pruning, sequential loading under a 20 s budget, filtering, paging) in `crates/server/src/routes/sessions/history.rs` (depends on T002, T004)
- [x] T006 Register `mod history;` and `.route("/history", get(history::get_session_history))` in `crates/server/src/routes/sessions/mod.rs` (depends on T005)
- [x] T007 Unit tests for bounds parsing/validation, overlap, effective-time fallback, paging (asc/desc/offset past end/has_more), and the turn-projection filter (keyword beyond the truncation cap, roles, window) in `crates/server/src/routes/sessions/history.rs` (depends on T005)

## Phase 3: MCP tool
- [x] T008 Add `SearchSessionMessagesRequest`, mirrored `SessionHistoryPayload`, and the `search_session_messages` tool (scope check, then GET `/history`); add earlier-turns pointers to the `list_recent_messages`/`list_all_messages` descriptions in `crates/mcp/src/task_server/tools/sessions.rs` (depends on the T006 contract)
- [x] T009 [P] Add `search_session_messages` to the expected orchestrator tool set in `crates/mcp/src/task_server/tools/mod.rs` (depends on T008)
- [x] T010 [P] Document the tool in `crates/mcp/AGENTS.md` under "Reading a session before following up"

## Phase 4: Validation
- [x] T011 Run `cargo test -p server --lib routes::` and `cargo test -p mcp`; `cargo clippy -p server -p mcp`; `pnpm run format`; confirm `shared/types.ts` unchanged (depends on T001–T010)
