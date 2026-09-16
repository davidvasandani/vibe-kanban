# Tasks: Recover Wedged MCP Sessions

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** are parallel-safe only within
their phase/layer.

## Phase 1: Recovery contracts

- [x] T001 Add recovery/discovery DTOs, safe errors, compatibility defaults, and
  unit tests in `crates/executors/src/mcp_recovery.rs`,
  `crates/executors/src/lib.rs`, and `crates/executors/src/mcp_refresh.rs`.
- [x] T002 [P] Reuse the existing affinity-bound execution transport for
  restart continuations, avoiding a second cluster protocol.
- [x] T003 Register generated recovery types in
  `crates/server/src/bin/generate_types.rs` and regenerate `shared/types.ts`
  (depends on T001).

## Phase 2: Backend lifecycle ownership

- [x] T004 Extend the generation-based refresh coordinator with restart,
  deadline-bounded terminalization, per-server evidence, and tests in
  `crates/services/src/services/mcp_refresh.rs` (depends on T001).
- [x] T005 Generalize restart reservation/service inputs and idempotency in
  `crates/services/src/services/queued_message.rs` (depends on T001).
- [x] T006 Add session/workspace recovery methods to
  `crates/services/src/services/container.rs` (depends on T001, T004, T005).
- [x] T007 Implement local session restart, fresh discovery observation,
  deadline transitions, and tests in
  `crates/local-deployment/src/container.rs` and capture Claude Code's
  `system/init.tools` registry evidence in
  `crates/executors/src/executors/claude.rs` (depends on T004-T006).
- [x] T008 Implement workspace-owned process-group restart and durable-action
  preservation in the workspace recovery route (depends on T007).
- [x] T009 Preserve affinity by dispatching restart continuations through the
  existing worker execution path and carrying restart state across process
  teardown (depends on T002, T006-T008).

## Phase 3: HTTP and MCP tools

- [x] T010 Refactor the existing browser restart route onto the container
  operation and add session/workspace restart/status routes with tests in
  `crates/server/src/routes/sessions/queue.rs`,
  `crates/server/src/routes/workspaces/mcp_refresh.rs`, and
  `crates/server/src/routes/workspaces/mod.rs` (depends on T006-T009).
- [x] T011 Add scoped `restart_session` and `restart_workspace` MCP tools and
  tool-inventory/default-context tests in
  `crates/mcp/src/task_server/tools/sessions.rs`,
  `crates/mcp/src/task_server/tools/workspaces.rs`, and
  `crates/mcp/src/task_server/tools/mod.rs` (depends on T010).
- [x] T012 Confirm `CLAUDE_CODE` refresh is terminally unsupported and update
  remediation/tests in `crates/executors/src/mcp_refresh.rs`,
  `crates/local-deployment/src/container.rs`, and
  `crates/server/src/routes/workspaces/mcp_refresh.rs` (depends on T007).

## Phase 4: Runtime status and issue UX

- [x] T013 Reuse the existing MCP refresh status endpoint/hook for generation-
  safe recovery status polling and expand its registry-state tests (depends on
  T003, T010).
- [x] T014 Add active-registry states and restart feedback to
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
  with focused component/model tests (depends on T013).
- [x] T015 [P] Extend safe diagnostic bundle/Markdown generation and tests in
  `packages/web-core/src/shared/lib/mcpDebugIssue.ts` and
  `packages/web-core/src/shared/lib/mcpDebugIssue.test.ts` (depends on T003).
- [x] T016 Wire terminal failure and panel-vs-registry disagreement to an
  explicit, deduplicated Debug issue action in the session toolbar (depends on
  T013-T015).

## Phase 5: Verification and documentation

- [x] T017 [P] Document both MCP tools, runtime registry semantics, bounded
  failure, and Claude Code refresh behavior in `crates/mcp/AGENTS.md` and the
  appropriate file under `docs/integrations/` (depends on T011-T016).
- [x] T018 Run `pnpm install --frozen-lockfile`, formatting, focused Rust/UI
  tests, generated-type checks, and `pnpm run check`. ESLint and clippy pass;
  the final lint guard reports six pre-existing unrelated
  `metricsDiskAlerts.*` locale keys (depends on T001-T017).
- [x] T019 Run independent Codex diff review, address confirmed significant
  findings, rerun affected verification, and repeat until clean (depends on
  T018).
- [x] T020 Distill reusable knowledge tagged `vk/38a6-expose-session-w`, update
  `docs/knowledge-base/INDEX.md`, and commit the knowledge base (depends on
  T019).
- [ ] T021 Open a pull request against the base branch, monitor required checks,
  fix any failures, and merge it (depends on T020).
