// @vitest-environment jsdom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  PROJECT_WORKSPACES_SHAPE,
  SINGLE_ISSUE_SHAPE,
  type Workspace,
} from 'shared/remote-types';
import {
  WorkspaceContext,
  type WorkspaceContextValue,
} from '@/shared/hooks/useWorkspaceContext';
import { useShape } from '@/shared/integrations/electric/hooks';
import { workspacesApi } from '@/shared/lib/api';
import { LinkedIssueProvider } from './LinkedIssueContext';

vi.mock('@/shared/integrations/electric/hooks', () => ({ useShape: vi.fn() }));
vi.mock('@/shared/hooks/auth/useAuth', () => ({
  useAuth: () => ({ isSignedIn: state.signedIn }),
}));
vi.mock('@/shared/lib/api', () => ({ workspacesApi: { update: vi.fn() } }));

const state = vi.hoisted(() => ({ signedIn: true }));
const remote = (id: string, issueId = 'issue-1', archived = true): Workspace =>
  ({
    id: `remote-${id}`,
    local_workspace_id: id,
    issue_id: issueId,
    archived,
  }) as Workspace;

// Only the identity lists are consumed by this provider.
function context(active = ['local-1'], archived: string[] = []) {
  return {
    activeWorkspaces: active.map((id) => ({ id })),
    archivedWorkspaces: archived.map((id) => ({ id })),
  } as WorkspaceContextValue;
}

describe('workspace drawer archive reconciliation', () => {
  let root: Root;
  let rows: Workspace[];
  let statusId: string;
  let localContext: WorkspaceContextValue | null;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    root = createRoot(document.createElement('div'));
    rows = [];
    statusId = 'in-progress';
    localContext = context();
    state.signedIn = true;
    vi.mocked(workspacesApi.update)
      .mockReset()
      .mockResolvedValue({} as never);
    vi.mocked(useShape)
      .mockReset()
      .mockImplementation(
        (shape) =>
          ({
            data:
              shape === PROJECT_WORKSPACES_SHAPE
                ? rows
                : shape === SINGLE_ISSUE_SHAPE
                  ? [{ id: 'issue-1', status_id: statusId }]
                  : [],
            isLoading: false,
            update: vi.fn(),
          }) as ReturnType<typeof useShape>
      );
  });

  afterEach(() => {
    act(() => root.unmount());
    globalThis.IS_REACT_ACT_ENVIRONMENT = false;
    vi.restoreAllMocks();
  });

  async function render(
    issueId: string | null = 'issue-1',
    projectId: string | null = 'project-1'
  ) {
    await act(async () => {
      root.render(
        <WorkspaceContext.Provider value={localContext}>
          <LinkedIssueProvider issueId={issueId} projectId={projectId}>
            <span>Workspace drawer</span>
          </LinkedIssueProvider>
        </WorkspaceContext.Provider>
      );
    });
  }

  it('waits for persisted remote archive state after an optimistic Done update', async () => {
    rows = [remote('local-1', 'issue-1', false)];
    await render();
    statusId = 'done';
    await render();
    expect(workspacesApi.update).not.toHaveBeenCalled();

    // Failed optimistic mutation rolls back without a local archive.
    statusId = 'in-progress';
    await render();
    expect(workspacesApi.update).not.toHaveBeenCalled();

    statusId = 'done';
    rows = [remote('local-1')];
    await render();
    expect(workspacesApi.update).toHaveBeenCalledTimes(1);
    expect(workspacesApi.update).toHaveBeenCalledWith('local-1', {
      archived: true,
    });

    // The local workspace stream moves the workspace to View Archive.
    localContext = context([], ['local-1']);
    await render();
    expect(workspacesApi.update).toHaveBeenCalledTimes(1);
  });

  it('reconciles an already archived snapshot on mount without ProjectProvider', async () => {
    rows = [remote('local-1')];
    await render();
    expect(workspacesApi.update).toHaveBeenCalledWith('local-1', {
      archived: true,
    });
  });

  it('archives all linked active workspaces but excludes unrelated and archived ones', async () => {
    localContext = context(['local-1', 'local-2', 'unrelated'], ['archived']);
    rows = [
      remote('local-1'),
      remote('local-2'),
      remote('unrelated', 'other-issue'),
      remote('archived'),
      remote('other-host'),
    ];
    await render();
    expect(vi.mocked(workspacesApi.update).mock.calls).toEqual([
      ['local-1', { archived: true }],
      ['local-2', { archived: true }],
    ]);
  });

  it('deduplicates in-flight writes across snapshots', async () => {
    let finish!: () => void;
    vi.mocked(workspacesApi.update).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = () => resolve({} as never);
        })
    );
    rows = [remote('local-1')];
    await render();
    rows = [...rows];
    await render();
    expect(workspacesApi.update).toHaveBeenCalledTimes(1);
    await act(async () => finish());
  });

  it('retries a failed archive on a later remote snapshot', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    vi.mocked(workspacesApi.update).mockRejectedValueOnce(new Error('offline'));
    rows = [remote('local-1')];
    await render();
    rows = [...rows];
    await render();
    expect(workspacesApi.update).toHaveBeenCalledTimes(2);
  });

  it.each([
    'missing-context',
    'missing-issue',
    'missing-project',
    'signed-out',
  ])('does not reconcile when %s', async (scenario) => {
    rows = [remote('local-1')];
    if (scenario === 'missing-context') localContext = null;
    if (scenario === 'signed-out') state.signedIn = false;
    await render(
      scenario === 'missing-issue' ? null : 'issue-1',
      scenario === 'missing-project' ? null : 'project-1'
    );
    expect(workspacesApi.update).not.toHaveBeenCalled();
    expect(useShape).toHaveBeenCalledWith(
      PROJECT_WORKSPACES_SHAPE,
      expect.anything(),
      { enabled: false }
    );
  });

  it('never unarchives on issue reopening', async () => {
    localContext = context([], ['local-1']);
    rows = [remote('local-1', 'issue-1', false)];
    await render();
    expect(workspacesApi.update).not.toHaveBeenCalled();
  });
});
