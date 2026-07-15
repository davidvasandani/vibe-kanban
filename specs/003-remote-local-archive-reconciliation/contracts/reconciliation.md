# Contract - Workspace archive reconciliation

This is an internal web-core contract. No HTTP endpoint, database schema,
generated type, or public API contract changes.

## Inputs

### Remote workspace snapshot

Source: `ProjectProvider` via `PROJECT_WORKSPACES_SHAPE`.

```ts
import type { Workspace as RemoteWorkspace } from 'shared/remote-types';
```

Required fields:

```ts
type RemoteWorkspaceArchiveLink = Pick<
  RemoteWorkspace,
  'id' | 'local_workspace_id' | 'archived'
>;
```

### Local workspace archive state

Source: `WorkspaceContext` from `WorkspaceProvider`.

```ts
type LocalWorkspaceArchiveState = {
  id: string;
  archived: boolean;
};
```

The hook may derive this by combining:
- `activeWorkspaces.map((w) => ({ id: w.id, archived: false }))`
- `archivedWorkspaces.map((w) => ({ id: w.id, archived: true }))`

If the same local ID appears in both lists during a stream transition, archived
state should win so reconciliation does not submit a redundant archive request.

## Pure selector contract

```ts
export function selectLocalWorkspaceIdsToArchive(
  remoteWorkspaces: RemoteWorkspaceArchiveLink[],
  localWorkspaces: LocalWorkspaceArchiveState[]
): string[];
```

Returns:
- local workspace IDs to archive;
- unique IDs only;
- stable order by first eligible remote row encountered.

Selection rules:
- Include when `remote.archived === true`.
- Include only when `remote.local_workspace_id` is present.
- Include only when a matching local workspace exists.
- Include only when matching local workspace `archived === false`.
- Ignore active remote rows.
- Ignore remote-only rows.
- Ignore already archived local rows.
- Never select IDs for unarchive.

## Hook contract

```ts
export function useRemoteLocalArchiveReconciliation(args: {
  remoteWorkspaces: RemoteWorkspaceArchiveLink[];
  localWorkspaces: LocalWorkspaceArchiveState[];
  enabled?: boolean;
}): void;
```

Behaviour:
- On each relevant snapshot change, compute selector output.
- If `enabled === false`, or local workspace state is unavailable to the caller,
  submit no archive requests.
- For every selected local workspace ID not already in flight, call:

```ts
workspacesApi.update(localWorkspaceId, { archived: true });
```

- Track in-flight requests by local workspace ID.
- Submit at most one in-flight request per local workspace ID.
- Run archive attempts independently; failure for one ID does not stop attempts
  for other IDs.
- Catch/log failures so unhandled promise rejections do not escape the provider.
- Remove each ID from the in-flight set when its request settles.
- Do not call `workspacesApi.update` with `{ archived: false }`.

## Non-goals

- No new remote mutation.
- No local endpoint change.
- No generated TypeScript change.
- No UI prop or rendering contract change.
- No persistent failed-attempt state.
- No requirement that every `ProjectProvider` mount is inside
  `WorkspaceProvider`; callers without local workspace state disable
  reconciliation.
