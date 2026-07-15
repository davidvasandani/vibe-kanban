# Data Model - Remote-to-local workspace archive reconciliation

No schema change. This feature connects existing remote workspace rows to
existing local workspace summaries.

## Remote `Workspace`

Generated in `shared/remote-types.ts` from the remote crate.

Relevant fields:
- `id: string` - remote workspace row ID.
- `project_id: string` - project owning the remote row.
- `issue_id: string | null` - linked issue, if any.
- `local_workspace_id: string | null` - ID of the corresponding local SQLite
  workspace on a paired host.
- `archived: boolean` - remote archive state streamed over Electric/fallback.
- `updated_at: string` - useful for debugging; not needed for selection.

Remote terminal issue archival changes only `archived` on remote rows. That
write remains owned by the remote issue mutation transaction.

## Local sidebar workspace state

Produced by `packages/web-core/src/shared/hooks/useWorkspaces.ts` and exposed by
`WorkspaceContext`.

Relevant fields on `SidebarWorkspace`:
- `id: string` - local workspace ID; matches remote `local_workspace_id`.
- `isArchived?: boolean` - local archive state derived from
  `WorkspaceWithStatus.archived`.

The hook receives local data as two collections:
- `activeWorkspaces: SidebarWorkspace[]`
- `archivedWorkspaces: SidebarWorkspace[]`

For selection, these can be normalized into:

```ts
type LocalWorkspaceArchiveState = {
  id: string;
  archived: boolean;
};
```

## Derived reconciliation mismatch

A local workspace is eligible for automatic archive when all conditions hold:

1. At least one remote workspace row has `archived === true`.
2. That remote row has a non-empty `local_workspace_id`.
3. A local workspace exists with `id === remote.local_workspace_id`.
4. The local workspace archive state is active (`archived === false`).
5. No archive request for that local workspace ID is currently in flight.

Duplicate remote rows that point to the same local workspace collapse to one
local workspace ID.

## State transition

```text
remote workspace.archived = true
remote workspace.local_workspace_id = local workspace.id
local workspace.archived = false
        |
        v
workspacesApi.update(local_workspace_id, { archived: true })
        |
        v
local workspace stream eventually moves the workspace from active to archived
```

No reverse transition is derived. A remote active workspace never causes
`workspacesApi.update(id, { archived: false })`.

## Persistence and lifecycle

The feature adds only transient client state:
- an in-memory in-flight set keyed by local workspace ID.

The in-flight set:
- prevents duplicate requests while the same mismatch remains visible;
- removes each ID when its request settles;
- is reset on provider remount, allowing retry.

