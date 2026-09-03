# Tasks: Refresh Active Workspace MCP Inventories

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked `[P]` touch independent files and may
run in parallel within their layer.

## Phase 1: Establish the failing boundary

- [ ] T001 Audit the restart launch chain and record whether Codex starts a new
  app-server and reads current MCP config before thread start/fork in
  `specs/vk/d71c-refresh-active-w/research.md`,
  `crates/server/src/routes/sessions/queue.rs`,
  `crates/services/src/services/queued_message.rs`, and
  `crates/executors/src/executors/codex.rs`.
- [ ] T002 Audit the internal live-refresh chain and status claims in
  `crates/services/src/services/mcp_refresh.rs`,
  `crates/local-deployment/src/container.rs`,
  `crates/worker/src/execution.rs`, and
  `crates/executors/src/executors/codex/client.rs` (depends on T001).
- [ ] T003 [P] Audit MCP management/restart wording and status sources in
  `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`,
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`,
  and `packages/web-core/src/i18n/locales/*/settings.json`.

## Phase 2: Add exact regression contracts

- [ ] T004 Add a deterministic Codex/app-server inventory fixture that exposes
  complete tool definitions and can change generations in
  `crates/executors/src/executors/codex/client.rs` and its nearest existing test
  module (depends on T001, T002).
- [ ] T005 Add stdio addition, removal, and same-name input-schema replacement
  assertions at the next-turn/fresh-process boundary in
  `crates/executors/src/executors/codex/client.rs` and/or
  `crates/executors/src/executors/codex.rs` (depends on T004).
- [ ] T006 [P] Add a streamable-HTTP materialization regression in
  `crates/executors/src/shared_mcp_config.rs` (depends on T001).
- [ ] T007 [P] Extend restart handoff tests for latest-config/fresh-process
  semantics in
  `packages/web-core/src/features/workspace-chat/model/restartAgentForMcpChanges.test.ts`
  and, if the server boundary changes,
  `crates/server/src/routes/sessions/queue.rs` (depends on T001).

## Phase 3: Correct demonstrated gaps

- [ ] T008 Implement the smallest correction required to ensure a fresh agent
  process adopts current stdio inventory in
  `crates/executors/src/executors/codex.rs`,
  `crates/server/src/routes/sessions/queue.rs`, and/or
  `crates/services/src/services/queued_message.rs` (depends on T005, T007; omit
  code changes where the audit proves the path already correct).
- [ ] T009 Correct any false-success or stale-snapshot behavior found in the
  internal refresh path in `crates/services/src/services/mcp_refresh.rs`,
  `crates/local-deployment/src/container.rs`,
  `crates/worker/src/execution.rs`, and
  `crates/executors/src/executors/codex/client.rs` (depends on T002, T005; omit
  code changes if no defect is demonstrated).
- [ ] T010 Correct only demonstrated Vibe Kanban status/wording conflation in
  `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`,
  `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`,
  and locale files, with focused tests beside the changed component (depends on
  T003).

## Phase 4: Verification and delivery

- [ ] T011 Run locked dependency setup, focused Rust/web tests, type generation
  checks when applicable, formatting, lint/check, and broader relevant tests;
  record commands and results in
  `specs/vk/d71c-refresh-active-w/verification.md` (depends on T005-T010).
- [ ] T012 Cross-check the final spec, plan, tasks, and diff against the
  constitution and record the result in
  `specs/vk/d71c-refresh-active-w/analysis.md` (depends on T011).
- [ ] T013 Run independent Codex diff review, address confirmed findings, repeat
  affected verification, and record the clean result in
  `specs/vk/d71c-refresh-active-w/review.md` (depends on T012).
- [ ] T014 Update `docs/knowledge-base/active-mcp-refresh.md` and
  `docs/knowledge-base/INDEX.md` with reusable shipped knowledge, or record “no
  new knowledge to record,” then commit it (depends on T013).
- [ ] T015 Verify the latest base, push the task branch, open a pull request, wait
  for required checks, address failures, and merge it (depends on T014).
