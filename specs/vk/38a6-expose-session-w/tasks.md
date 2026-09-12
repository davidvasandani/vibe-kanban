# Tasks: Recover Wedged MCP Sessions

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** are parallel-safe only within
their phase/layer.

## Phase 1: Recovery contracts

- [ ] T001 Add recovery/discovery DTOs, safe errors, compatibility defaults, and
  unit tests in `crates/executors/src/mcp_recovery.rs`,
  `crates/executors/src/lib.rs`, and `crates/executors/src/mcp_refresh.rs`.
- [ ] T002 [P] Add cluster restart request/result compatibility contracts in
  `crates/cluster-protocol/src/lib.rs`.
- [ ] T003 Register generated recovery types in
  `crates/server/src/bin/generate_types.rs` and regenerate `shared/types.ts`
  (depends on T001).

## Phase 2: Backend lifecycle ownership

- [ ] T004 Add a generation-based, deadline-bounded recovery coordinator and
  tests in `crates/services/src/services/mcp_recovery.rs` and
  `crates/services/src/services/mod.rs` (depends on T001).
- [ ] T005 Generalize restart reservation/service inputs and idempotency tests in
  `crates/services/src/services/queued_message.rs` (depends on T001).
- [ ] T006 Add session/workspace recovery methods to
  `crates/services/src/services/container.rs` (depends on T001, T004, T005).
- [ ] T007 Implement local session restart, fresh discovery observation,
  deadline transitions, and tests in
  `crates/local-deployment/src/container.rs` and capture Claude Code's
  `system/init.tools` registry evidence in
  `crates/executors/src/executors/claude.rs` (depends on T004-T006).
- [ ] T008 Implement workspace-owned process restart/preservation and tests in
  `crates/local-deployment/src/container.rs` (depends on T007).
- [ ] T009 Add affinity-bound worker restart operations and rolling-compatible
  tests in `crates/worker/src/execution.rs`, `crates/worker/src/worker_api.rs`,
  `crates/services/src/services/cluster/client.rs`, and
  `crates/local-deployment/src/container.rs` (depends on T002, T006-T008).

## Phase 3: HTTP and MCP tools

- [ ] T010 Refactor the existing browser restart route onto the container
  operation and add session/workspace restart/status routes with tests in
  `crates/server/src/routes/sessions/queue.rs`,
  `crates/server/src/routes/workspaces/mcp_refresh.rs`, and
  `crates/server/src/routes/workspaces/mod.rs` (depends on T006-T009).
- [ ] T011 Add scoped `restart_session` and `restart_workspace` MCP tools and
  tool-inventory/default-context tests in
  `crates/mcp/src/task_server/tools/sessions.rs`,
  `crates/mcp/src/task_server/tools/workspaces.rs`, and
  `crates/mcp/src/task_server/tools/mod.rs` (depends on T010).
- [ ] T012 Confirm `CLAUDE_CODE` refresh is terminally unsupported and update
  remediation/tests in `crates/executors/src/mcp_refresh.rs`,
  `crates/local-deployment/src/container.rs`, and
  `crates/server/src/routes/workspaces/mcp_refresh.rs` (depends on T007).

## Phase 4: Runtime status and issue UX

- [ ] T013 Add recovery/status API clients and a race-safe polling hook with
  tests in `packages/web-core/src/shared/lib/api.ts`,
  `packages/web-core/src/features/workspace-chat/model/useMcpRecovery.ts`, and
  `packages/web-core/src/features/workspace-chat/model/useMcpRecovery.test.ts`
  (depends on T003, T010).
- [ ] T014 Add active-registry states and restart feedback to
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
  with focused component/model tests (depends on T013).
- [ ] T015 [P] Extend safe diagnostic bundle/Markdown generation and tests in
  `packages/web-core/src/shared/lib/mcpDebugIssue.ts` and
  `packages/web-core/src/shared/lib/mcpDebugIssue.test.ts` (depends on T003).
- [ ] T016 Wire terminal failure and panel-vs-registry disagreement to the
  existing explicit Debug issue action in
  `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
  and locale files under `packages/web-core/src/i18n/locales/` (depends on
  T013-T015).

## Phase 5: Verification and documentation

- [ ] T017 [P] Document both MCP tools, runtime registry semantics, bounded
  failure, and Claude Code refresh behavior in `crates/mcp/AGENTS.md` and the
  appropriate file under `docs/integrations/` (depends on T011-T016).
- [ ] T018 Run `pnpm install --frozen-lockfile`, formatting, focused Rust/UI
  tests, generated-type checks, `pnpm run check`, and `pnpm run lint`; fix all
  regressions (depends on T001-T017).
- [ ] T019 Run independent Codex diff review, address confirmed significant
  findings, rerun affected verification, and repeat until clean (depends on
  T018).
- [ ] T020 Distill reusable knowledge tagged `vk/38a6-expose-session-w`, update
  `docs/knowledge-base/INDEX.md`, and commit the knowledge base (depends on
  T019).
- [ ] T021 Open a pull request against the base branch, monitor required checks,
  fix any failures, and merge it (depends on T020).
