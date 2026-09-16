/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { WorkspaceSummary, WorkspaceWithStatus } from 'shared/types';
import { useWorkspaces, type UseWorkspacesResult } from './useWorkspaces';
import { workspaceSummaryKeys } from './workspaceSummaryKeys';

const request = vi.hoisted(() => vi.fn());
const host = vi.hoisted(() => ({ id: null as string | null }));
vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: request,
}));
vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => host.id,
}));
vi.mock('@/shared/hooks/useJsonPatchWsStream', () => ({
  useJsonPatchWsStream: (endpoint: string) => ({
    data: {
      workspaces: {
        one: {
          id: 'one',
          name: 'Workspace',
          branch: 'branch',
          pinned: true,
          archived: endpoint.endsWith('archived=true'),
          is_running: false,
          creation_status: 'ready',
          created_at: '2026-09-16T00:00:00Z',
          updated_at: '2026-09-16T00:00:00Z',
        } as WorkspaceWithStatus,
      },
    },
    isConnected: true,
    isInitialized: true,
    error: null,
  }),
}));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function summary(overrides: Partial<WorkspaceSummary> = {}): WorkspaceSummary {
  return {
    workspace_id: 'one',
    latest_session_id: 'session',
    has_pending_approval: true,
    files_changed: 3,
    lines_added: 20,
    lines_removed: 4,
    latest_process_completed_at: '2026-09-16T00:00:00Z',
    latest_process_status: 'completed',
    has_running_dev_server: true,
    has_running_poller: true,
    has_unseen_turns: true,
    pr_status: 'open',
    pr_number: 42n,
    pr_url: 'https://example.com/pull/42',
    affinity: {
      kind: 'local',
      placement_state: 'ready',
      worker_node_id: null,
      worker_hostname: null,
      requested_worker_node_id: null,
      requested_worker_hostname: null,
    } as WorkspaceSummary['affinity'],
    ...overrides,
  };
}

function response(summaries: WorkspaceSummary[]) {
  return {
    ok: true,
    json: async () => ({ success: true, data: { summaries } }),
  };
}

let client: QueryClient;
let root: ReturnType<typeof createRoot>;
let latest: UseWorkspacesResult;
function Probe() {
  latest = useWorkspaces();
  return null;
}
async function flush(ms = 10) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}
async function render() {
  await act(async () => {
    root.render(
      <QueryClientProvider client={client}>
        <Probe />
      </QueryClientProvider>
    );
  });
  await flush();
}
async function refresh() {
  await act(async () => {
    await client.invalidateQueries({ queryKey: workspaceSummaryKeys.all });
  });
  await flush();
}

beforeEach(() => {
  vi.useFakeTimers();
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  root = createRoot(document.createElement('div'));
  request.mockResolvedValue(response([summary()]));
});
afterEach(() => {
  act(() => root.unmount());
  client.clear();
  request.mockReset();
  host.id = null;
  vi.useRealTimers();
});

const failures = [
  ['HTTP', () => Promise.resolve({ ok: false, status: 503 })],
  ['network', () => Promise.reject(new Error('offline'))],
  [
    'API',
    () => Promise.resolve({ ok: true, json: async () => ({ success: false }) }),
  ],
  [
    'missing data',
    () => Promise.resolve({ ok: true, json: async () => ({ success: true }) }),
  ],
  [
    'invalid collection',
    () =>
      Promise.resolve({
        ok: true,
        json: async () => ({ success: true, data: { summaries: {} } }),
      }),
  ],
  [
    'invalid JSON',
    () =>
      Promise.resolve({
        ok: true,
        json: async () => {
          throw new SyntaxError('invalid JSON');
        },
      }),
  ],
] as const;

it.each(failures)(
  'retains active and archived metadata on %s failure and recovers on polling',
  async (_kind, fail) => {
    await render();
    const before = latest;
    expect(before.workspaces[0].prNumber).toBe(42);
    request.mockImplementation(fail);
    await refresh();
    expect(latest.workspaces).toEqual(before.workspaces);
    expect(latest.archivedWorkspaces).toEqual(before.archivedWorkspaces);
    for (const archived of [false, true]) {
      expect(
        client.getQueryState(workspaceSummaryKeys.byArchived(archived, null))
          ?.status
      ).toBe('error');
    }
    request.mockResolvedValue(
      response([summary({ files_changed: 7, pr_status: 'merged' })])
    );
    await flush(15000);
    expect(latest.workspaces[0]).toMatchObject({
      filesChanged: 7,
      prStatus: 'merged',
    });
    expect(latest.archivedWorkspaces[0].filesChanged).toBe(7);
  }
);

it('accepts explicit clears and successful empty snapshots', async () => {
  await render();
  request.mockResolvedValue(
    response([
      summary({
        files_changed: 0,
        lines_added: 0,
        lines_removed: 0,
        has_pending_approval: false,
        has_running_dev_server: false,
        has_running_poller: false,
        has_unseen_turns: false,
        latest_process_completed_at: undefined,
        latest_process_status: null,
        pr_status: null,
        pr_number: null,
        pr_url: null,
      }),
    ])
  );
  await refresh();
  expect(latest.workspaces[0]).toMatchObject({
    filesChanged: 0,
    hasPendingApproval: false,
    hasRunningPoller: false,
    hasUnseenActivity: false,
    prStatus: undefined,
    prNumber: undefined,
  });
  // A complete snapshot with no entry for this workspace removes enrichment.
  request.mockResolvedValue(response([summary({ workspace_id: 'another' })]));
  await refresh();
  expect(latest.workspaces[0].filesChanged).toBeUndefined();
  request.mockResolvedValue(response([]));
  await refresh();
  expect(latest.workspaces[0]).toMatchObject({
    name: 'Workspace',
    isPinned: true,
    isRunning: false,
    filesChanged: undefined,
  });
  expect(latest.archivedWorkspaces[0].prNumber).toBeUndefined();
  expect(
    client.getQueryState(workspaceSummaryKeys.byArchived(false, null))?.status
  ).toBe('success');
});

it('keeps active and archived query outcomes independent', async () => {
  await render();
  request.mockImplementation(async (_url: string, init: RequestInit) => {
    if (JSON.parse(init.body as string).archived)
      throw new Error('archive unavailable');
    return response([summary({ files_changed: 9 })]);
  });
  await refresh();
  expect(latest.workspaces[0].filesChanged).toBe(9);
  expect(latest.archivedWorkspaces[0].filesChanged).toBe(3);
});

it('keeps initial failures retryable without inventing metadata', async () => {
  request.mockRejectedValue(new Error('offline'));
  await render();
  expect(latest.workspaces[0].filesChanged).toBeUndefined();
  expect(
    client.getQueryState(workspaceSummaryKeys.byArchived(false, null))?.status
  ).toBe('error');
  request.mockResolvedValue(response([summary()]));
  await flush(15000);
  expect(latest.workspaces[0].filesChanged).toBe(3);
});

it('isolates host switches and late responses even when workspace IDs match', async () => {
  await render();
  let finishOldHost!: (value: ReturnType<typeof response>) => void;
  const pending = new Promise<ReturnType<typeof response>>((resolve) => {
    finishOldHost = resolve;
  });
  request.mockReturnValue(pending);
  const oldRefresh = client.invalidateQueries({
    queryKey: workspaceSummaryKeys.all,
  });
  await flush();
  host.id = 'second-host';
  request.mockRejectedValue(new Error('new host unavailable'));
  await render();
  expect(latest.workspaces[0].prNumber).toBeUndefined();
  expect(latest.archivedWorkspaces[0].prNumber).toBeUndefined();
  await act(async () => {
    finishOldHost(response([summary({ files_changed: 99 })]));
    await oldRefresh;
  });
  await flush();
  expect(latest.workspaces[0].filesChanged).toBeUndefined();
  request.mockResolvedValue(response([summary({ files_changed: 8 })]));
  await flush(15000);
  expect(latest.workspaces[0].filesChanged).toBe(8);
  expect(request).toHaveBeenCalledWith(
    '/api/host/second-host/workspaces/summaries',
    expect.anything()
  );
});
