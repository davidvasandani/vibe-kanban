# Technical Plan: Remote-to-local workspace archive reconciliation

**Feature dir**: `specs/003-remote-local-archive-reconciliation/`
**Task**: `vk/f464-vk-workspace-mgm`
**Spec**: [`spec.md`](spec.md)

## Approach

Add a small shared web-core reconciliation path that observes current remote
project workspace rows and current local workspace lists. When a remote row is
archived and linked to an active local workspace, submit the existing local
archive update for that local workspace.

This is intentionally client-side convergence after the remote transaction has
already committed and streamed. The existing remote issue mutation remains
unchanged and transaction/txid-covered.

Grounding (real files):
- `packages/web-core/src/shared/providers/remote/ProjectProvider.tsx`
  subscribes to `PROJECT_WORKSPACES_SHAPE`; invoke the reconciliation hook here.
- `packages/web-core/src/shared/providers/WorkspaceProvider.tsx`
  exposes local `activeWorkspaces` and `archivedWorkspaces` through
  `WorkspaceContext`.
- `ProjectProvider` also appears in command/dialog flows. Read
  `WorkspaceContext` through the exported nullable context, or pass local lists
  from a caller that already has them, so missing local workspace context disables
  reconciliation rather than throwing.
- `packages/web-core/src/shared/hooks/useWorkspaces.ts`
  defines `SidebarWorkspace` and local active/archived stream semantics.
- `packages/web-core/src/shared/lib/api.ts`
  exposes `workspacesApi.update(id, { archived: true })`.
- `shared/remote-types.ts` and `shared/types.ts`
  already contain the required generated remote/local workspace fields.

See [`research.md`](research.md), [`data-model.md`](data-model.md), and
[`contracts/reconciliation.md`](contracts/reconciliation.md).

## Changes

1. Add a new web-core reconciliation module, for example
   `packages/web-core/src/shared/providers/remote/useRemoteLocalArchiveReconciliation.ts`.
2. In that module, export a pure selector
   `selectLocalWorkspaceIdsToArchive(remoteWorkspaces, localWorkspaces)` that
   returns unique local workspace IDs whose remote row is archived while the
   local workspace is still active.
3. In the same module, export a hook
   `useRemoteLocalArchiveReconciliation({ remoteWorkspaces, localWorkspaces, enabled })`
   that:
   - keeps an in-flight `Set<string>` in a ref;
   - calls `workspacesApi.update(id, { archived: true })` for selected IDs not
     in flight;
   - catches/logs per-workspace failures;
   - removes IDs from the in-flight set in `finally`.
4. Update `ProjectProvider` to derive local archive state only when
   `WorkspaceContext` is present, using `useContext(WorkspaceContext)` rather
   than the throwing `useWorkspaceContext()` hook, and call the reconciliation
   hook with `enabled && Boolean(workspaceContext)`.
5. Add focused web-core tests:
   - selector unit tests for archived linked rows, remote-only rows, already
     archived locals, active remotes, duplicate links, and multiple eligible
     workspaces;
   - reconciler/hook-equivalent tests for in-flight deduplication, independent
     failures, disabled/missing local context, and retry after settlement.

## Data model

No schema or generated-type change. Relevant existing fields:
- Remote `Workspace.local_workspace_id: string | null`
- Remote `Workspace.archived: boolean`
- Local `SidebarWorkspace.id: string`
- Local `SidebarWorkspace.isArchived?: boolean`

Normalize local state into `LocalWorkspaceArchiveState[]` before selection.
When active and archived streams temporarily contain the same local ID, treat the
workspace as archived to avoid redundant archive calls.

## Contracts

No public API change. Internal contract is documented in
[`contracts/reconciliation.md`](contracts/reconciliation.md).

The only network call added by this feature is the existing local endpoint call
already wrapped by:

```ts
workspacesApi.update(localWorkspaceId, { archived: true });
```

## Constitution check

- **I Clarity** - pure selector plus small effect hook; no hidden issue-panel
  side effects. OK
- **II Test the contract** - selector tests and reconciliation dispatch tests
  cover the behavioural contract before/with implementation. OK
- **III Small, reversible steps** - frontend-only module plus one provider
  invocation; no schema/API/type generation. OK
- **IV Shared-component boundaries** - no presentational component changes;
  shared provider logic owns data convergence. OK
- **V Remote mutations are transactional and txid-covered** - remote issue
  mutation behaviour stays untouched; reconciliation reacts only after remote
  rows are visible. OK
- **VI Don't rebuild what shipped** - reuses `PROJECT_WORKSPACES_SHAPE`,
  `WorkspaceContext`, and `workspacesApi.update`. OK

## Risks

- Provider assumption: `ProjectProvider` needs local workspace context. Current
  app shells render workspace providers above project routes, but several
  command/dialog flows instantiate `ProjectProvider` directly. Reconciliation
  must use a nullable context read or caller-supplied local lists and disable
  itself when local workspace data is absent.
- Local stream lag: after `workspacesApi.update`, active/archived streams may
  take a moment to move the workspace. The in-flight set suppresses duplicate
  submissions during that gap.
- Retry cadence: failures retry only after data changes or remount, not on an
  internal timer. This matches the spec and avoids hidden request loops.

## Verification

- `pnpm --filter @vibe/web-core run test`
- `pnpm run web-core:check` or full `pnpm run check`
- `pnpm run format`
- Inspect `git diff -- specs/003-remote-local-archive-reconciliation` for this
  planning task; implementation should later inspect full code diff.
