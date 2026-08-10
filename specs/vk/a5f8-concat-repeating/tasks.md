# Tasks: Background Workspace Creation

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and may run together within their layer.

## Phase 1: Persistent lifecycle contract

- [ ] T001 Add workspace creation status/error columns with existing rows defaulting to ready in `crates/db/migrations/<timestamp>_workspace_creation_status.sql`.
- [ ] T002 Add `WorkspaceCreationStatus`, fields, atomic claim/terminal transition helpers, unfinished lookup/reconciliation helpers, and model tests in `crates/db/src/models/workspace.rs` (depends on T001).
- [ ] T003 Change `CreateAndStartWorkspaceResponse` to an acceptance response in `crates/db/src/models/requests.rs` and update generated declarations through `crates/server/src/bin/generate_types.rs` into `shared/types.ts` (depends on T002).

## Phase 2: Request-independent backend workflow

- [ ] T004 Refactor validation and the existing slow create/start body into a workspace-scoped background runner in `crates/server/src/routes/workspaces/create.rs`; persist queued before returning, atomically claim, spawn under Tokio ownership, record ready/failed, and preserve placement/import/analytics semantics (depends on T002–T003).
- [ ] T005 Add request-independent lifecycle tests covering pre-acceptance validation, early response, single-consumer claim, success, safe failure text, and no duplicate initial execution in `crates/server/src/routes/workspaces/create.rs` and/or a focused test module (depends on T004).
- [ ] T006 Add conservative startup reconciliation for queued/running workspaces using initial-execution evidence in `crates/local-deployment/src/lib.rs` or the existing startup/container reconciliation module, with focused database/service tests (depends on T002, T004).
- [ ] T007 Update the MCP task-attempt create-and-start caller to accept a workspace, wait/resolve the resulting initial execution through existing APIs, and never issue a second start in `crates/mcp/src/task_server/tools/task_attempts.rs` (depends on T003–T004).

## Phase 3: Frontend convergence

- [ ] T008 [P] Add creation-state presentation copy to relevant locale files under `packages/web-core/src/i18n/locales/*` (depends on T003 field semantics).
- [ ] T009 Update workspace record/list types and refresh behavior plus create mutations/direct callers in `packages/web-core/src/shared/hooks/useCreateWorkspace.ts`, `packages/web-core/src/shared/hooks/useWorkspaceRecord.ts`, `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`, and `packages/web-core/src/pages/workspaces/VSCodeWorkspacePage.tsx` (depends on T003–T004).
- [ ] T010 Add a shared workspace creation pending/failed state and guard session/repository-assuming content in `packages/web-core/src/pages/workspaces/WorkspacesMainContainer.tsx`, `packages/web-core/src/pages/workspaces/VSCodeWorkspacePage.tsx`, and the kanban workspace panel boundary as required (depends on T008–T009).
- [ ] T011 Add focused rendered tests for queued/running, ready, and failed workspace states in the nearest existing `packages/web-core/src/**/*.test.tsx` files (depends on T010).

## Phase 4: Verification and handoff

- [ ] T012 Run `pnpm install --frozen-lockfile`, focused Rust tests, focused frontend tests, and `pnpm run generate-types:check`; fix task-scoped failures (depends on T005–T007, T011).
- [ ] T013 Run `pnpm run format`, `pnpm run check`, and applicable lint; fix task-scoped failures (depends on T012).
- [ ] T014 Run independent Codex diff review, address confirmed significant findings, re-run affected checks, and repeat until clean (depends on T013).
- [ ] T015 Update `docs/knowledge-base/` and `docs/knowledge-base/INDEX.md` with reusable request-independent lifecycle guidance tagged `vk/5e1e-vk-workspace-cre`, then commit the knowledge-base update (depends on T014).
- [ ] T016 Commit the complete implementation and merge branch `vk/5e1e-vk-workspace-cre` into its recorded base branch (depends on T015).
