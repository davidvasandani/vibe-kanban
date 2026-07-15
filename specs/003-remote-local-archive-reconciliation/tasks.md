# Tasks: Remote-to-local workspace archive reconciliation

**Plan**: [`plan.md`](plan.md)
**Task**: `vk/f464-vk-workspace-mgm`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
or only run read-only verification commands and may run in parallel within
their layer.

## Layer 1 - Reconciliation Module
- [x] T001 Create `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.ts` with narrow remote/local archive input types matching the contract.
- [x] T002 Add `selectLocalWorkspaceIdsToArchive` in `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.ts`; return unique local IDs in first eligible remote-row order, ignoring remote-only, active-remote, and already archived local rows. (depends on T001)
- [x] T003 Add local workspace archive-state normalization in `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.ts`; combine active and archived local workspace lists with archived state winning on duplicate IDs. (depends on T001)
- [x] T004 Add the reconciliation hook/reconciler in `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.ts`; track in-flight local IDs, call `workspacesApi.update(id, { archived: true })`, isolate per-workspace failures, log/catch errors, and clean up in `finally`. (depends on T002, T003)

## Layer 2 - Provider Wiring
- [x] T005 Wire the reconciliation hook into `packages/web-core/src/shared/providers/remote/ProjectProvider.tsx` using `workspacesResult.data` and local active/archived workspace lists from a nullable `WorkspaceContext` read; do not call the throwing `useWorkspaceContext()` hook from `ProjectProvider`, and disable reconciliation when local workspace context is absent. (depends on T004)

## Layer 3 - Focused Tests
- [x] T006 Add selector unit tests in `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.test.ts` for archived linked rows, remote-only rows, already archived locals, active remotes, duplicate remote links, no auto-unarchive, and multiple eligible workspaces. (depends on T002, T003)
- [x] T007 Add hook/reconciler-equivalent tests in `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.test.ts` for in-flight deduplication, independent failure isolation, disabled/missing-local-context behavior, and retry after settlement. (depends on T004, T006)

## Layer 4 - Verification
- [x] T008 [P] Run focused web-core tests, for example `pnpm --filter @vibe/web-core run test`. (depends on T006, T007)
- [x] T009 [P] Run frontend type checking, using `pnpm run web-core:check` if available or full `pnpm run check`. (depends on T005)
- [x] T010 Run `pnpm run format`. (depends on T008, T009)
- [x] T011 Inspect the final diff and confirm no generated files, remote transaction files, docs outside this feature directory, or unrelated changes were modified. (depends on T010)

## Definition of Done
All acceptance criteria in [`spec.md`](spec.md) hold; no backend, schema,
endpoint, or generated-type changes are present; focused tests and frontend type
checks pass; `pnpm run format` has been run.
