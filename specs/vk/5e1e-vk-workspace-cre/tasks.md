# Tasks: Background Workspace Creation

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and may run together within their layer.

## Phase 1: Persistent lifecycle contract

- [x] T001 Add workspace creation status/error columns with existing rows defaulting to ready in `crates/db/migrations/20260810000000_workspace_creation_status.sql`.
- [x] T002 Add `WorkspaceCreationStatus`, fields, atomic claim/terminal transition helpers, unfinished reconciliation helpers, and model tests in `crates/db/src/models/workspace.rs` (depends on T001).
- [x] T003 Change `CreateAndStartWorkspaceResponse` to an acceptance response in `crates/db/src/models/requests.rs`, register the enum in `crates/server/src/bin/generate_types.rs`, and regenerate `shared/types.ts` (depends on T002).

## Phase 2: Request-independent backend workflow

- [x] T004 Refactor validation and the slow create/start body into a workspace-scoped background runner in `crates/server/src/routes/workspaces/create.rs`; persist queued before returning, atomically claim, spawn under Tokio ownership, record ready/failed, and preserve placement/import/analytics semantics (depends on T002–T003).
- [x] T005 Add lifecycle tests covering the single-consumer claim, terminal success, late-failure refusal, and restart reconciliation in `crates/db/src/models/workspace.rs`; verify the handler returns immediately after spawning the detached runner (depends on T004).
- [x] T006 Add conservative startup reconciliation for queued/running workspaces in `crates/server/src/main.rs` and `crates/server/src/startup.rs`, with focused model tests (depends on T002, T004).
- [x] T007 Audit the MCP task-attempt caller in `crates/mcp/src/task_server/tools/task_attempts.rs`; it already consumes only the returned workspace identity and requires no execution response (depends on T003–T004).

## Phase 3: Frontend convergence

- [x] T008 [P] Add creation-state presentation copy to locale files under `packages/web-core/src/i18n/locales/*` (depends on T003 field semantics).
- [x] T009 Update workspace-detail refresh behavior in `packages/web-core/src/shared/hooks/useWorkspaceRecord.ts`; audit create mutations/direct callers, which already consume only `workspace` (depends on T003–T004).
- [x] T010 Add a shared pending/failed guard above session/repository-assuming content in `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx` and `WorkspaceCreationStatusView.tsx` (depends on T008–T009).
- [x] T011 Add rendered tests for queued/running, ready, and failed states in `packages/web-core/src/pages/workspaces/WorkspaceCreationStatusView.test.tsx` (depends on T010).

## Phase 4: Verification and handoff

- [x] T012 Run locked dependency install, focused Rust tests, focused frontend tests, and generated-type/SQLx checks (depends on T005–T007, T011).
- [x] T013 Run formatting, repository-wide checks, and applicable web-core lint (depends on T012).
- [x] T014 Run independent Codex diff review, address confirmed findings, re-run affected checks, and repeat until clean (depends on T013).
- [x] T015 Update `docs/knowledge-base/` and `docs/knowledge-base/INDEX.md` with reusable request-independent lifecycle guidance tagged `vk/5e1e-vk-workspace-cre`, then commit it (depends on T014).
- [x] T016 Commit the final UI test extraction and merge branch `vk/5e1e-vk-workspace-cre` into its recorded base branch (depends on T015).
