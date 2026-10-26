import { useEffect, useRef } from 'react';
import type { Workspace as RemoteWorkspace } from 'shared/remote-types';
import { workspacesApi } from '@/shared/lib/api';

export type RemoteWorkspaceArchiveLink = Pick<
  RemoteWorkspace,
  'id' | 'local_workspace_id' | 'archived'
>;

export type LocalWorkspaceArchiveState = {
  id: string;
  archived: boolean;
};

type LocalWorkspaceIdentity = {
  id: string;
};

type ReconcileArchiveArgs = {
  remoteWorkspaces: RemoteWorkspaceArchiveLink[];
  localWorkspaces: LocalWorkspaceArchiveState[];
  inFlightLocalWorkspaceIds: Set<string>;
  archiveAutomatically: (localWorkspaceId: string) => Promise<unknown>;
  enabled?: boolean;
  onError?: (localWorkspaceId: string, error: unknown) => void;
};

export function normalizeLocalWorkspaceArchiveState(
  activeWorkspaces: LocalWorkspaceIdentity[],
  archivedWorkspaces: LocalWorkspaceIdentity[]
): LocalWorkspaceArchiveState[] {
  const localWorkspacesById = new Map<string, LocalWorkspaceArchiveState>();

  for (const workspace of activeWorkspaces) {
    localWorkspacesById.set(workspace.id, {
      id: workspace.id,
      archived: false,
    });
  }

  for (const workspace of archivedWorkspaces) {
    localWorkspacesById.set(workspace.id, {
      id: workspace.id,
      archived: true,
    });
  }

  return Array.from(localWorkspacesById.values());
}

export function selectLocalWorkspaceIdsToArchive(
  remoteWorkspaces: RemoteWorkspaceArchiveLink[],
  localWorkspaces: LocalWorkspaceArchiveState[]
): string[] {
  const localWorkspacesById = new Map(
    localWorkspaces.map((workspace) => [workspace.id, workspace])
  );
  const selectedLocalWorkspaceIds = new Set<string>();
  const localWorkspaceIdsToArchive: string[] = [];

  for (const remoteWorkspace of remoteWorkspaces) {
    if (!remoteWorkspace.archived || !remoteWorkspace.local_workspace_id) {
      continue;
    }

    const localWorkspace = localWorkspacesById.get(
      remoteWorkspace.local_workspace_id
    );
    if (!localWorkspace || localWorkspace.archived) {
      continue;
    }

    if (!selectedLocalWorkspaceIds.has(localWorkspace.id)) {
      selectedLocalWorkspaceIds.add(localWorkspace.id);
      localWorkspaceIdsToArchive.push(localWorkspace.id);
    }
  }

  return localWorkspaceIdsToArchive;
}

export function reconcileRemoteLocalArchiveState({
  remoteWorkspaces,
  localWorkspaces,
  inFlightLocalWorkspaceIds,
  archiveAutomatically,
  enabled = true,
  onError = (localWorkspaceId, error) => {
    console.warn(
      `Failed to reconcile archived remote workspace to local workspace ${localWorkspaceId}:`,
      error
    );
  },
}: ReconcileArchiveArgs): void {
  if (!enabled) {
    return;
  }

  const localWorkspaceIdsToArchive = selectLocalWorkspaceIdsToArchive(
    remoteWorkspaces,
    localWorkspaces
  );

  for (const localWorkspaceId of localWorkspaceIdsToArchive) {
    if (inFlightLocalWorkspaceIds.has(localWorkspaceId)) {
      continue;
    }

    inFlightLocalWorkspaceIds.add(localWorkspaceId);
    archiveAutomatically(localWorkspaceId)
      .catch((error: unknown) => {
        onError(localWorkspaceId, error);
      })
      .finally(() => {
        inFlightLocalWorkspaceIds.delete(localWorkspaceId);
      });
  }
}

export function useRemoteLocalArchiveReconciliation({
  remoteWorkspaces,
  localWorkspaces,
  enabled = true,
}: {
  remoteWorkspaces: RemoteWorkspaceArchiveLink[];
  localWorkspaces: LocalWorkspaceArchiveState[];
  enabled?: boolean;
}): void {
  const inFlightLocalWorkspaceIdsRef = useRef(new Set<string>());

  useEffect(() => {
    const reconcile = () =>
      reconcileRemoteLocalArchiveState({
        remoteWorkspaces,
        localWorkspaces,
        inFlightLocalWorkspaceIds: inFlightLocalWorkspaceIdsRef.current,
        archiveAutomatically: workspacesApi.archiveAutomatically,
        enabled,
      });
    reconcile();
    if (!enabled) return;
    // A blocked request remains remote intent; retry even without a new shape
    // snapshot when the local agent/poller/wake-up finishes.
    const timer = setInterval(reconcile, 30_000);
    return () => clearInterval(timer);
  }, [enabled, localWorkspaces, remoteWorkspaces]);
}
