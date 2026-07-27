# Tasks: Refresh MCP Tools in Active Workspace Sessions

**Feature**: `specs/vk/8c27-refresh-mcp-tool/`  
**Task**: `8c27-refresh-mcp-tool`

Tasks are dependency-ordered. `[P]` tasks in the same layer touch independent
files and may be completed together.

## Layer 1 — Domain and protocol contracts

- [ ] T001 Add typed refresh statuses, safe errors, server snapshots, and
  executor-control abstractions in
  `crates/executors/src/mcp_refresh.rs` and
  `crates/executors/src/lib.rs`.
- [ ] T002 Add the process-local per-session claim/confirm state machine,
  last-known-good merge, and atomic generation publication with unit tests in
  `crates/services/src/services/mcp_refresh.rs` and
  `crates/services/src/services/mod.rs`. Depends on T001.
- [ ] T003 [P] Add safe allow-listed error mapping/redaction tests in
  `crates/executors/src/mcp_refresh.rs`. Depends on T001.
- [ ] T004 Add Codex app-server `config/mcpServer/reload` and fully paginated
  status methods plus mapping tests in
  `crates/executors/src/executors/codex/client.rs`. Depends on T001.

## Layer 2 — Live executor/container handoff

- [ ] T005 Extend `SpawnedChild` with the optional live MCP control handoff and
  initialize it for Codex in
  `crates/executors/src/executors/mod.rs` and
  `crates/executors/src/executors/codex.rs`. Depends on T004.
- [ ] T006 Register/remove live controls, store refresh state, implement
  `ContainerService` refresh/status operations, and confirm pending generations
  on the next Codex execution in
  `crates/services/src/services/container.rs` and
  `crates/local-deployment/src/container.rs`. Depends on T002, T005.
- [ ] T007 Add container lifecycle/concurrency tests for pending-next-turn,
  active-call non-interruption, duplicate busy, control teardown, unsupported
  executors, and atomic confirmation in
  `crates/local-deployment/src/container.rs`. Depends on T006.

## Layer 3 — Public backend and generated contracts

- [ ] T008 Add GET/POST workspace-session refresh routes and route tests in
  `crates/server/src/routes/workspaces/mcp_refresh.rs` and
  `crates/server/src/routes/workspaces/mod.rs`. Depends on T006.
- [ ] T009 Register refresh types and regenerate checked-in TypeScript/schema
  outputs via `crates/server/src/bin/generate_types.rs`, `shared/types.ts`, and
  `shared/schemas/`. Depends on T001, T008.
- [ ] T010 [P] Add `refresh_mcp_tools` request/response client support in
  `crates/mcp/src/task_server/tools/mod.rs` and
  `crates/mcp/src/task_server/tools/sessions.rs`. Depends on T008.
- [ ] T011 Add the scoped/global VK MCP tool, router membership tests, and safe
  result rendering in `crates/mcp/src/task_server/tools/sessions.rs` and
  `crates/mcp/src/task_server/tools/mod.rs`. Depends on T010.

## Layer 4 — Web UX

- [ ] T012 Add refresh/status API client methods in
  `packages/web-core/src/shared/lib/api.ts`. Depends on T009.
- [ ] T013 [P] Add localized Refresh MCP tools/status/error strings to
  `packages/web-core/src/i18n/locales/*/*.json`. Depends on T009.
- [ ] T014 Add the active-session refresh control, pending polling, confirmed
  timestamp, and per-server detail UI in the relevant
  `packages/web-core/src` workspace/session components. Depends on T012, T013.
- [ ] T015 Add rendered-DOM tests for pending, confirmed, partial, busy,
  unsupported, unknown counts/restart, and stale-response suppression beside the
  selected web component. Depends on T014.

## Layer 5 — Protocol and regression coverage

- [ ] T016 [P] Add deterministic stdio fixtures/tests for tool addition/removal,
  malformed `tools/list`, timeout, partial failure, and secret redaction under
  `crates/executors/tests/` or the closest existing MCP test module. Depends on
  T006.
- [ ] T017 [P] Add deterministic streamable-HTTP fixtures/tests for the same
  refresh behaviors under `crates/executors/tests/` or the closest existing MCP
  test module. Depends on T006.
- [ ] T018 Add the ignored isolated-cache Slack `v1.3.0-vk.2` live Codex
  regression verifying `attachment_get_data` and unchanged session identity in
  the closest executor integration test module. Depends on T011, T016.
- [ ] T019 [P] Document the supported-executor contract, pending-next-turn
  semantics, statuses, and remediation in
  `docs/integrations/mcp-server-configuration.mdx`. Depends on T014.

## Layer 6 — Verification

- [ ] T020 Install locked frontend dependencies with
  `pnpm install --frozen-lockfile`, then run focused Rust and frontend tests.
  Depends on T007, T011, T015, T016, T017.
- [ ] T021 [P] Run generated-type/schema checks and relevant TypeScript checks.
  Depends on T009, T015.
- [ ] T022 [P] Run relevant Rust workspace checks/tests. Depends on T020.
- [ ] T023 Run `pnpm run format`, rerun affected validation, and inspect the diff
  for unrelated/generated changes. Depends on T019, T020, T021, T022.

## Layer 7 — Independent review and knowledge

- [ ] T024 Run independent Codex diff review, address confirmed significant
  findings, rerun relevant validation, and repeat until clean. Depends on T023.
- [ ] T025 Update `docs/knowledge-base/` and its index with reusable live MCP
  refresh architecture tagged `8c27-refresh-mcp-tool`, then commit the knowledge
  base. Depends on T024.

## Parallel execution notes

- T003 and T004 may proceed independently after T001.
- T010 can proceed alongside generated-contract work once the REST shape exists,
  but T011 waits for its client contract.
- T013 is independent of API-client implementation.
- T016, T017, and T019 touch independent test/documentation surfaces.
- T021 and T022 are independent read-only verification after their prerequisites.
