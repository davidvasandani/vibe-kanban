# Tasks: `list_all_messages`

**Plan**: `./plan.md`

Tasks are dependency-ordered. `[P]` marks work that is safe within the same
layer because it touches independent files.

## Layer 1 — Server selection contract

- [x] T001 Add explicit recent/all selection and `all` query parsing in
  `crates/server/src/routes/execution_processes.rs`.
- [x] T002 Update the session messages route to derive the shared selection in
  `crates/server/src/routes/sessions/mod.rs`. Depends on T001.
- [x] T003 Add focused >100-entry, role-filter, order, and `has_more` unit tests
  in `crates/server/src/routes/execution_processes.rs`. Depends on T001.

## Layer 2 — MCP tool

- [x] T004 Refactor shared message target resolution/authorization and HTTP
  request selection in `crates/mcp/src/task_server/tools/sessions.rs`. Depends
  on T001, T002.
- [x] T005 Add the `list_all_messages` request/tool implementation in
  `crates/mcp/src/task_server/tools/sessions.rs`. Depends on T004.
- [x] T006 [P] Add `list_all_messages` to the orchestrator exposure contract in
  `crates/mcp/src/task_server/tools/mod.rs`. Depends on T005.
- [x] T007 [P] Document recent-versus-all usage and the normalized projection
  boundary in `crates/mcp/AGENTS.md`. Depends on T005.

## Layer 3 — Verification

- [x] T008 [P] Run focused `server` tests for the response selection contract.
  Depends on T003.
- [x] T009 [P] Run focused `mcp` tests/checks for tool discovery and sessions.
  Depends on T005, T006.
- [x] T010 Run repository formatting, relevant broader Rust verification, and
  inspect the diff for generated or unrelated changes. Depends on T007-T009.

## Layer 4 — Review and knowledge

- [x] T011 Run independent Codex diff review, fix confirmed findings, rerun
  affected checks, and repeat until no significant findings remain. Depends on
  T010.
- [x] T012 Update the relevant project knowledge page and
  `docs/knowledge-base/INDEX.md`, tagged `vk/29d8-vk-list-all-mess`, and commit
  the knowledge base. Depends on T011.
- [x] T013 Commit implementation/spec artifacts, push the task branch, open a
  pull request against the base branch, wait for required checks, and merge.
  Depends on T012.

## Parallel execution notes

- T006 and T007 touch independent files after the MCP implementation lands.
- T008 and T009 are independent read-only verification commands and may run
  together after their respective code/tests are complete.
- The server builder and session route are sequential because T002 consumes the
  selection contract introduced by T001.
