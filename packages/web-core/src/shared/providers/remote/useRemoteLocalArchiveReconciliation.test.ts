import { describe, expect, it, vi } from 'vitest';
import {
  normalizeLocalWorkspaceArchiveState,
  reconcileRemoteLocalArchiveState,
  selectLocalWorkspaceIdsToArchive,
  type LocalWorkspaceArchiveState,
  type RemoteWorkspaceArchiveLink,
} from '@/shared/providers/remote/useRemoteLocalArchiveReconciliation';

function remoteWorkspace(
  overrides: Partial<RemoteWorkspaceArchiveLink>
): RemoteWorkspaceArchiveLink {
  return {
    id: 'remote-workspace',
    local_workspace_id: null,
    archived: false,
    ...overrides,
  };
}

function localWorkspace(
  id: string,
  archived = false
): LocalWorkspaceArchiveState {
  return { id, archived };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('selectLocalWorkspaceIdsToArchive', () => {
  it('selects archived remote rows linked to active local workspaces', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            id: 'remote-1',
            local_workspace_id: 'local-1',
            archived: true,
          }),
        ],
        [localWorkspace('local-1')]
      )
    ).toEqual(['local-1']);
  });

  it('ignores remote-only rows', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [remoteWorkspace({ archived: true })],
        [localWorkspace('local-1')]
      )
    ).toEqual([]);
  });

  it('ignores already archived local workspaces', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            local_workspace_id: 'local-1',
            archived: true,
          }),
        ],
        [localWorkspace('local-1', true)]
      )
    ).toEqual([]);
  });

  it('ignores active remote rows', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            local_workspace_id: 'local-1',
            archived: false,
          }),
        ],
        [localWorkspace('local-1')]
      )
    ).toEqual([]);
  });

  it('returns duplicate remote links once in first eligible remote-row order', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            id: 'remote-1',
            local_workspace_id: 'local-2',
            archived: true,
          }),
          remoteWorkspace({
            id: 'remote-2',
            local_workspace_id: 'local-1',
            archived: true,
          }),
          remoteWorkspace({
            id: 'remote-3',
            local_workspace_id: 'local-2',
            archived: true,
          }),
        ],
        [localWorkspace('local-1'), localWorkspace('local-2')]
      )
    ).toEqual(['local-2', 'local-1']);
  });

  it('does not select local unarchive candidates', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            local_workspace_id: 'local-1',
            archived: false,
          }),
        ],
        [localWorkspace('local-1', true)]
      )
    ).toEqual([]);
  });

  it('selects multiple eligible workspaces', () => {
    expect(
      selectLocalWorkspaceIdsToArchive(
        [
          remoteWorkspace({
            id: 'remote-1',
            local_workspace_id: 'local-1',
            archived: true,
          }),
          remoteWorkspace({
            id: 'remote-2',
            local_workspace_id: 'local-2',
            archived: true,
          }),
        ],
        [localWorkspace('local-1'), localWorkspace('local-2')]
      )
    ).toEqual(['local-1', 'local-2']);
  });
});

describe('normalizeLocalWorkspaceArchiveState', () => {
  it('combines active and archived local workspace lists with archived winning', () => {
    expect(
      normalizeLocalWorkspaceArchiveState(
        [{ id: 'local-1' }, { id: 'local-2' }],
        [{ id: 'local-2' }, { id: 'local-3' }]
      )
    ).toEqual([
      { id: 'local-1', archived: false },
      { id: 'local-2', archived: true },
      { id: 'local-3', archived: true },
    ]);
  });
});

describe('reconcileRemoteLocalArchiveState', () => {
  const remoteWorkspaces = [
    remoteWorkspace({
      local_workspace_id: 'local-1',
      archived: true,
    }),
  ];
  const localWorkspaces = [localWorkspace('local-1')];

  it('deduplicates repeated snapshots while a local archive is in flight', async () => {
    let resolveUpdate: () => void = () => {};
    const updateWorkspace = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveUpdate = resolve;
        })
    );
    const inFlightLocalWorkspaceIds = new Set<string>();

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds,
      updateWorkspace,
    });
    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds,
      updateWorkspace,
    });

    expect(updateWorkspace).toHaveBeenCalledTimes(1);
    expect(updateWorkspace).toHaveBeenCalledWith('local-1', {
      archived: true,
    });

    resolveUpdate();
    await flushPromises();

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds,
      updateWorkspace,
    });

    expect(updateWorkspace).toHaveBeenCalledTimes(2);
  });

  it('isolates failures so other eligible workspaces still archive', async () => {
    const updateWorkspace = vi
      .fn()
      .mockRejectedValueOnce(new Error('archive failed'))
      .mockResolvedValueOnce(undefined);
    const onError = vi.fn();

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces: [
        remoteWorkspace({
          id: 'remote-1',
          local_workspace_id: 'local-1',
          archived: true,
        }),
        remoteWorkspace({
          id: 'remote-2',
          local_workspace_id: 'local-2',
          archived: true,
        }),
      ],
      localWorkspaces: [localWorkspace('local-1'), localWorkspace('local-2')],
      inFlightLocalWorkspaceIds: new Set(),
      updateWorkspace,
      onError,
    });

    expect(updateWorkspace).toHaveBeenCalledTimes(2);
    expect(updateWorkspace).toHaveBeenNthCalledWith(1, 'local-1', {
      archived: true,
    });
    expect(updateWorkspace).toHaveBeenNthCalledWith(2, 'local-2', {
      archived: true,
    });

    await flushPromises();

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError).toHaveBeenCalledWith('local-1', expect.any(Error));
  });

  it('does nothing when reconciliation is disabled', () => {
    const updateWorkspace = vi.fn().mockResolvedValue(undefined);

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds: new Set(),
      updateWorkspace,
      enabled: false,
    });

    expect(updateWorkspace).not.toHaveBeenCalled();
  });

  it('does nothing when local workspace context is missing', () => {
    const updateWorkspace = vi.fn().mockResolvedValue(undefined);

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces: [],
      inFlightLocalWorkspaceIds: new Set(),
      updateWorkspace,
    });

    expect(updateWorkspace).not.toHaveBeenCalled();
  });

  it('retries a failed archive after the previous request settles', async () => {
    const updateWorkspace = vi
      .fn()
      .mockRejectedValueOnce(new Error('archive failed'))
      .mockResolvedValueOnce(undefined);
    const inFlightLocalWorkspaceIds = new Set<string>();

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds,
      updateWorkspace,
      onError: vi.fn(),
    });
    await flushPromises();

    reconcileRemoteLocalArchiveState({
      remoteWorkspaces,
      localWorkspaces,
      inFlightLocalWorkspaceIds,
      updateWorkspace,
      onError: vi.fn(),
    });

    expect(updateWorkspace).toHaveBeenCalledTimes(2);
    expect(updateWorkspace).toHaveBeenNthCalledWith(2, 'local-1', {
      archived: true,
    });
  });
});
