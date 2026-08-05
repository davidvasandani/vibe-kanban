# Tasks: Workspace Server Affinity and Migration

**Plan**: `./plan.md`

Tasks are dependency-ordered. Tasks marked **[P]** touch independent files and may be executed together within their layer.

## Phase 1: Backend discovery and durable transition design

- [ ] T001 Confirm the existing container follow-up, placement provisioning, execution idempotency, and per-workspace synchronization seams; record the chosen durable claim design in `specs/vk/9a64-vk-workspace-aff/research.md`.
- [ ] T002 Define the final request/response/error and summary Rust types in `crates/api-types/src/workspace_affinity.rs`, `crates/api-types/src/lib.rs`, and/or the owning server modules, and register exports in `crates/server/src/bin/generate_types.rs` (depends on T001).
- [ ] T003 Implement or extend the durable affinity migration operation/idempotency storage selected by T001 in `crates/db/migrations/*`, `crates/db/src/models/workspace_affinity_migration.rs`, and `crates/db/src/models/mod.rs`; include DB tests (depends on T001).

## Phase 2: Placement and continuation primitives

- [ ] T004 [P] Add compare-and-set workspace reassignment and placement-read helpers with unit tests in `crates/db/src/models/workspace.rs` (depends on T002).
- [ ] T005 [P] Add exact running coding-agent/persistent-process lookup helpers and executor-config extraction tests in `crates/db/src/models/execution_process.rs` (depends on T002).
- [ ] T006 Extract a reusable coordinator-owned follow-up builder/start service from `crates/server/src/routes/sessions/mod.rs` into `crates/services/src/services/container.rs` or a focused service module, preserving current session, working-directory, cleanup, and executor validation behavior; retain route behavior and tests (depends on T005).
- [ ] T007 Add a reusable placement/re-provision operation that calls the existing `WorkerScheduler` rules in `crates/services/src/services/cluster/scheduler.rs`, `crates/local-deployment/src/container.rs`, and the focused affinity service as needed (depends on T004).

## Phase 3: Managed affinity service and route

- [ ] T008 Implement the source-owned migration continuation prompt, keyed workspace claim, lifecycle sequencing, idempotent outcome replay, and precise durable-boundary errors in `crates/services/src/services/workspace_affinity.rs`, `crates/services/src/services/mod.rs`, and service-container wiring (depends on T003–T007).
- [ ] T009 Add stopped update, confirmation-required, persistent-process conflict, ambiguous execution, stop failure, successful restart, restart partial failure, stale placement, and duplicate-operation tests beside `crates/services/src/services/workspace_affinity.rs` (depends on T008).
- [ ] T010 Wire `PATCH /api/workspaces/{id}/affinity` in `crates/server/src/routes/workspaces/affinity.rs` and `crates/server/src/routes/workspaces/mod.rs`, with typed actionable HTTP errors and route tests (depends on T008).

## Phase 4: Bulk summary and generated contracts

- [ ] T011 Extend bulk workspace summaries with resolved affinity using one worker inventory read in `crates/server/src/routes/workspaces/workspace_summary.rs`; add summary-kind tests (depends on T002, T004).
- [ ] T012 Regenerate `shared/types.ts` with `pnpm run generate-types` and verify it with `pnpm run generate-types:check` (depends on T010, T011).

## Phase 5: Shared frontend data layer

- [ ] T013 [P] Add affinity API client and host-scoped query/mutation/cache helpers in `packages/web-core/src/shared/lib/api.ts`, `packages/web-core/src/shared/hooks/useWorkspaceAffinity.ts`, and `packages/web-core/src/shared/hooks/workspaceSummaryKeys.ts` (depends on T012).
- [ ] T014 [P] Extract shared worker eligibility/options and affinity-label helpers with unit tests in `packages/web-core/src/shared/lib/workerPlacement.ts` and update `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` to use them (depends on T012).
- [ ] T015 [P] Map bulk affinity summaries into `SidebarWorkspace` in `packages/web-core/src/shared/hooks/useWorkspaces.ts` with focused mapper tests (depends on T012).

## Phase 6: Left drawer and right drawer UI

- [ ] T016 [P] Add compact server-affinity presentation to `packages/ui/src/components/WorkspacesSidebar.tsx` and `packages/ui/src/components/WorkspaceSummary.tsx`, with component tests in the existing `packages/remote-web/src/test/` suite (depends on T015).
- [ ] T017 [P] Add `PERSIST_KEYS.serverAffinitySection` and its union member in `packages/web-core/src/shared/stores/useUiPreferencesStore.ts` (depends on T012).
- [ ] T018 Implement the Server Affinity body, Run on selector, provisional running-target confirmation dialog, pending/partial-error UI, and cache convergence in `packages/web-core/src/pages/workspaces/ServerAffinitySectionContainer.tsx` plus focused tests (depends on T013, T014).
- [ ] T019 Insert the Server Affinity accordion before Server Metrics in `packages/web-core/src/pages/workspaces/RightSidebar.tsx` with section-order/default-collapse tests (depends on T017, T018).
- [ ] T020 Add all new strings to `packages/web-core/src/i18n/locales/*/common.json` and update i18n completeness tests if present (depends on T016, T018).

## Phase 7: Verification and hardening

- [ ] T021 Run focused backend tests for DB, affinity service, summaries, and routes; fix failures in their owning files (depends on T009–T012).
- [ ] T022 [P] Run focused frontend unit/component tests for worker options, workspace mapping, left drawer, and right drawer; fix failures in their owning files (depends on T016–T020).
- [ ] T023 [P] Verify stopped and running migrations manually in the local UI for local, automatic, eligible worker, unavailable current worker, cancel, success, and restart-failed displays; record results in `specs/vk/9a64-vk-workspace-aff/validation.md` (depends on T016–T020).
- [ ] T024 Run `pnpm install --frozen-lockfile` if needed, `pnpm run format`, `pnpm run generate-types:check`, `pnpm run check`, relevant Rust tests, and `pnpm run lint`; fix all task-caused failures (depends on T021–T023).

## Phase 8: Review and knowledge capture

- [ ] T025 Run independent `codex review` against the task diff, record review passes in `specs/vk/9a64-vk-workspace-aff/review.md`, fix confirmed significant findings, and repeat T021–T024 until review is clear (depends on T024).
- [ ] T026 Add/update the reusable workspace affinity/migration page under `docs/knowledge-base/`, tag it `9a64-vk-workspace-aff`, refresh `docs/knowledge-base/INDEX.md`, and commit the knowledge-base change (depends on T025).
