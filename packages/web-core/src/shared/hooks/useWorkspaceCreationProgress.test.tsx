/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, it, vi } from 'vitest';
import type { Workspace, WorkspaceCreationProgress } from 'shared/types';
import { useWorkspaceCreationProgress } from './useWorkspaceCreationProgress';

const getProgress = vi.hoisted(() => vi.fn());
const host = vi.hoisted(() => ({ id: null as string | null }));
vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => host.id,
}));
vi.mock('@/shared/lib/api', () => ({
  workspacesApi: { getCreationProgress: getProgress },
}));
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  vi.useRealTimers();
  getProgress.mockReset();
  host.id = null;
});

function snapshot(
  id: string,
  status: WorkspaceCreationProgress['status'] = 'running'
): WorkspaceCreationProgress {
  return { workspace_id: id, status, phase: 'worktrees', updated_at: null };
}

it('isolates workspace queries, rehydrates on remount, and stops polling terminal state', async () => {
  vi.useFakeTimers();
  getProgress.mockImplementation(async (id: string) => snapshot(id));
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const container = document.createElement('div');
  const root = createRoot(container);
  let latest: WorkspaceCreationProgress | undefined;
  function Probe({
    id,
    status = 'running',
  }: {
    id: string;
    status?: Workspace['creation_status'];
  }) {
    const query = useWorkspaceCreationProgress({
      id,
      creation_status: status,
    } as Workspace);
    latest = query.data;
    return null;
  }
  async function render(
    id: string,
    status: Workspace['creation_status'] = 'running'
  ) {
    await act(async () => {
      root.render(
        <QueryClientProvider client={client}>
          <Probe key={id} id={id} status={status} />
        </QueryClientProvider>
      );
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
  }
  try {
    await render('one');
    expect(latest?.workspace_id).toBe('one');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(getProgress).toHaveBeenCalledTimes(2);
    host.id = 'second-host';
    await render('one');
    expect(getProgress).toHaveBeenCalledTimes(3);
    await render('two');
    expect(latest?.workspace_id).toBe('two');
    getProgress.mockImplementation(async (id: string) =>
      snapshot(id, 'failed')
    );
    await render('one');
    expect(latest?.status).toBe('failed');
    const calls = getProgress.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(getProgress).toHaveBeenCalledTimes(calls);
    await render('ready', 'ready');
    expect(getProgress).toHaveBeenCalledTimes(calls);
  } finally {
    act(() => root.unmount());
    client.clear();
  }
});
