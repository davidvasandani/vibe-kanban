# Research - Remote-to-local workspace archive reconciliation

**Feature dir**: `specs/003-remote-local-archive-reconciliation/`
**Inputs**: [`spec.md`](spec.md), repository inspection, `PRIOR_KNOWLEDGE.md`,
`IMPLEMENTATION_PLAN.md`, `.specify/memory/constitution.md`

## Existing behaviour

Remote issue updates already archive linked remote workspace rows when an issue
enters a terminal status. Prior knowledge records that
`archive_workspaces_for_terminal_issue` runs from both single and bulk issue
update paths in `crates/remote/src/routes/issues.rs`, inside the existing
Postgres transaction and before the returned txid is captured. That behaviour is
correct for the remote system and remains out of scope for this feature.

The gap is cross-boundary convergence: after the remote workspace row has
`archived = true`, the connected local host can still expose the corresponding
local workspace in its active workspace stream.

## Repo findings

- `packages/web-core/src/shared/providers/remote/ProjectProvider.tsx`
  subscribes to `PROJECT_WORKSPACES_SHAPE` and exposes project-scoped remote
  `Workspace[]` through `ProjectContext`. The provider receives both Electric
  live updates and fallback snapshots through `useShape`.
- `packages/web-core/src/shared/providers/remote/UserProvider.tsx` subscribes to
  `USER_WORKSPACES_SHAPE`. It is user-scoped, but the feature is project-panel
  reconciliation, and `ProjectProvider` already owns the project workspace
  collection needed by issue views.
- `packages/web-core/src/shared/providers/WorkspaceProvider.tsx` calls
  `useWorkspaces()` and exposes `activeWorkspaces` and `archivedWorkspaces` in
  `WorkspaceContext`.
- Normal project routes render `ProjectProvider` under `WorkspaceProvider`, but
  several command/dialog flows also instantiate `ProjectProvider` directly for
  project-scoped data. Because `useWorkspaceContext()` throws when the context
  is absent, `ProjectProvider` should read the exported nullable
  `WorkspaceContext` with React `useContext` or receive local lists from a
  caller that has already read the context. Missing local workspace state should
  disable reconciliation, not crash the provider.
- `packages/web-core/src/shared/hooks/useWorkspaces.ts` builds local sidebar
  workspace lists from two local WebSocket streams:
  `/api/workspaces/streams/ws?archived=false` and
  `/api/workspaces/streams/ws?archived=true`. The local list shape used by
  providers is `SidebarWorkspace`, with `id` and `isArchived`.
- `packages/web-core/src/shared/lib/api.ts` exposes
  `workspacesApi.update(workspaceId, { archived?: boolean; pinned?: boolean; name?: string })`.
  Existing UI archive actions already use `workspacesApi.update(id, { archived: true })`.
- Remote workspace rows are generated in `shared/remote-types.ts` as
  `Workspace = { id, project_id, owner_user_id, issue_id, local_workspace_id,
  name, archived, ... }`.
- Local workspace rows are generated in `shared/types.ts` as
  `Workspace = { id, ..., archived, pinned, ... }`, while the provider-facing
  sidebar summary uses `isArchived`.

## Decisions

### Decision: Reconcile in `ProjectProvider` through a dedicated web-core hook

Add a small hook under `packages/web-core/src/shared/providers/remote/` or
`packages/web-core/src/shared/hooks/remote/`, and invoke it from
`ProjectProvider` after `workspacesResult` is created. The hook should read the
local workspace lists from a safe nullable `WorkspaceContext` read, or accept the
local lists as parameters from the provider after such a read.

Rationale:
- `ProjectProvider` already has the remote project workspace snapshot.
- It is rendered under `WorkspaceProvider` for normal project routes in both
  local and remote app shells, so local active/archived workspace lists are
  available there.
- Dialog/command mounts that lack local workspace context can safely skip
  reconciliation until mounted in a route that has local workspace data.
- It keeps issue panels, cards, and workspace UI presentational.
- A hook triggered from current snapshots covers Electric updates, fallback
  snapshots, reconnects, and provider remounts.

### Decision: Use level-triggered selection, not event history

The selector should inspect the current remote workspace collection and the
current local active/archived lists on each effect pass. It should not assume a
specific Electric event is delivered exactly once.

Rationale:
- Prior knowledge records Electric-first plus REST fallback delivery.
- Snapshot replacement and remounts should converge without special cases.
- This makes retry behaviour natural: if an archive attempt fails, later data
  changes or a remount can select the same mismatch again.

### Decision: Keep mismatch selection pure and exported

Create an exported helper similar to:

```ts
export function selectLocalWorkspaceIdsToArchive(
  remoteWorkspaces: RemoteWorkspace[],
  localWorkspaces: LocalWorkspaceArchiveState[]
): string[];
```

The helper should return unique local workspace IDs where:
- a remote workspace row has `archived === true`;
- that remote row has a non-empty `local_workspace_id`;
- a local workspace with that ID exists;
- the local workspace is not archived.

Rationale:
- Pure selection is easy to test thoroughly in the existing node Vitest setup.
- It prevents provider rendering details from obscuring the core contract.
- It gives the reconciliation hook one simple responsibility: dispatching
  independent local archive requests.

### Decision: Dispatch one independent request per eligible local workspace

The hook should maintain a `Set<string>` in a ref for in-flight local workspace
IDs. For each selected ID not already in flight, add it to the set, call
`workspacesApi.update(id, { archived: true })`, catch/log failures, and remove
the ID in `finally`.

Rationale:
- Prevents repeated requests while remote/local snapshots re-render.
- A failed archive for one local workspace does not cancel others.
- Removing IDs on settle preserves retryability without building persistent
  failure state.

### Decision: No schema, API, generated-type, or remote transaction changes

This feature uses the existing local workspace update endpoint and existing
remote workspace shape fields. It does not add migrations, generated type
changes, or remote mutation logic.

Rationale:
- The needed data already exists on both sides.
- The local update path owns process cleanup, archive script handling, DB flag
  mutation, stream updates, and any existing local-to-remote convergence.
- Constitution V requires remote mutation side effects to stay transaction-bound;
  this feature should consume the result of that mutation rather than moving it.

## Testing implications

- Add pure Vitest coverage for the selector. Existing `@vibe/web-core` tests run
  in a node environment and already cover pure logic.
- Add focused hook/effect coverage for in-flight deduplication and failure
  isolation. If React hook testing would require a new test dependency, avoid
  the dependency by exporting a small reconciler runner/factory around the
  in-flight set and testing it as pure async logic. Do not add a new dependency
  for this feature.
- Cover the disabled/missing-local-context path so a `ProjectProvider` mount
  without `WorkspaceProvider` cannot submit archive requests or throw from this
  feature path.
- Run `pnpm --filter @vibe/web-core run test` for the focused tests,
  `pnpm run web-core:check` or `pnpm run check` for types, and
  `pnpm run format` before completion.
