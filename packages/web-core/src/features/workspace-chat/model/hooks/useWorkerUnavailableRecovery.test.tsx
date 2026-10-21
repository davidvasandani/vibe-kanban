/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type WorkerNode,
} from 'shared/types';
import { ApiError } from '@/shared/lib/api';
import { RECOVERY_WINDOW_MS } from '@/shared/lib/workerRecovery';
import { useWorkerUnavailableRecovery } from './useWorkerUnavailableRecovery';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  listWorkers: vi.fn(),
  updateAffinity: vi.fn(),
}));

vi.mock('@/shared/lib/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/shared/lib/api')>()),
  workerNodesApi: { list: mocks.listWorkers },
  workspacesApi: { updateAffinity: mocks.updateAffinity },
}));

const PROFILE = 'CLAUDE_CODE';

function worker(id: string, refusing = false): WorkerNode {
  const now = Date.now();
  return {
    id,
    hostname: id,
    status: WorkerNodeStatus.online,
    worker_version: '1',
    vibe_version: '1',
    capabilities: { executor_profiles: [PROFILE] },
    resource_snapshot: {
      load_1m: 1,
      active_execution_count: 0,
      admission_refusing: refusing,
    },
    labels: {},
    mount_status: WorkerMountStatus.healthy,
    mount_message: null,
    last_heartbeat_at: new Date(now).toISOString(),
    // Far beyond the fake clock's advances in these tests.
    lease_expires_at: new Date(now + 24 * 60 * 60 * 1000).toISOString(),
    created_at: new Date(now).toISOString(),
    updated_at: new Date(now).toISOString(),
  };
}

function refusal(workerId = 'think4') {
  return new ApiError(
    'think4 is not accepting new executions',
    503,
    undefined,
    {
      worker_node_id: workerId,
      worker: workerId,
      reason: 'refusing',
      retryable: true,
    }
  );
}

type RecoveryHook = ReturnType<typeof useWorkerUnavailableRecovery>;
let hook: RecoveryHook;

function Harness(props: Parameters<typeof useWorkerUnavailableRecovery>[0]) {
  hook = useWorkerUnavailableRecovery(props);
  return null;
}

/** Let pending promise continuations (API mocks, state updates) run. */
async function flush() {
  await act(async () => {
    for (let i = 0; i < 5; i += 1) await Promise.resolve();
  });
}

async function advance(ms: number) {
  await act(async () => {
    vi.advanceTimersByTime(ms);
  });
  await flush();
}

describe('useWorkerUnavailableRecovery', () => {
  let container: HTMLDivElement;
  let root: Root;
  let resend: ReturnType<typeof vi.fn<(prompt: string) => Promise<boolean>>>;

  function render(sessionId = 'session') {
    const queryClient = new QueryClient();
    act(() =>
      root.render(
        <QueryClientProvider client={queryClient}>
          <Harness
            sessionId={sessionId}
            workspaceId="workspace"
            requiredExecutorProfile={PROFILE}
            resend={resend}
          />
        </QueryClientProvider>
      )
    );
  }

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(Date.parse('2026-10-21T12:00:00Z'));
    mocks.listWorkers.mockReset();
    mocks.updateAffinity.mockReset();
    resend = vi.fn<(prompt: string) => Promise<boolean>>();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('ignores errors that are not an unavailable execution server', () => {
    render();
    let started = true;
    act(() => {
      started = hook.start('hello', new ApiError('boom', 500), 'session');
    });
    expect(started).toBe(false);
    expect(hook.state).toBeNull();
  });

  it('ignores a refusal from a session that is no longer current', () => {
    render('current');
    let started = true;
    act(() => {
      started = hook.start('hello', refusal(), 'previous');
    });
    expect(started).toBe(false);
    expect(hook.state).toBeNull();
    expect(mocks.listWorkers).not.toHaveBeenCalled();
  });

  it('never auto-resends when the poll settles after the deadline', async () => {
    mocks.listWorkers.mockResolvedValue([worker('think4', true)]);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    // The next poll's request is slow: it resolves (with the server
    // accepting again) only after the window has closed.
    let resolveList: (workers: WorkerNode[]) => void = () => undefined;
    mocks.listWorkers.mockImplementation(
      () =>
        new Promise<WorkerNode[]>((resolve) => {
          resolveList = resolve;
        })
    );
    await advance(5_000);
    vi.setSystemTime(Date.now() + RECOVERY_WINDOW_MS);
    await act(async () => {
      resolveList([worker('think4')]);
    });
    await flush();

    expect(resend).not.toHaveBeenCalled();
    expect(hook.state?.phase).toBe('expired');
  });

  it('suggests eligible, non-refusing alternatives without resending', async () => {
    mocks.listWorkers.mockResolvedValue([
      // The refusing server's stale heartbeat must not trigger a resend on
      // the first look.
      worker('think4'),
      worker('think2', true),
      worker('think3'),
    ]);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    expect(hook.state?.phase).toBe('suggesting');
    expect(hook.state?.refusedWorker).toBe('think4');
    expect(hook.state?.alternatives.map((w) => w.id)).toEqual(['think3']);
    expect(resend).not.toHaveBeenCalled();
  });

  it('waits with exponential backoff and resends once the server recovers', async () => {
    mocks.listWorkers.mockResolvedValue([worker('think4', true)]);
    resend.mockResolvedValue(true);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    expect(hook.state?.phase).toBe('waiting');
    expect(hook.state?.nextCheckAt).toBe(Date.now() + 5_000);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(1);

    await advance(5_000);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(2);
    expect(hook.state?.nextCheckAt).toBe(Date.now() + 10_000);

    mocks.listWorkers.mockResolvedValue([worker('think4')]);
    await advance(9_999);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(2);
    await advance(1);
    expect(resend).toHaveBeenCalledWith('hello');
    await flush();
    expect(hook.state).toBeNull();
  });

  it('continues the same window when the resend is refused again', async () => {
    mocks.listWorkers.mockResolvedValue([worker('think4', true)]);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();
    const deadline = hook.state?.deadline;

    mocks.listWorkers.mockResolvedValue([worker('think4')]);
    resend.mockImplementation(async (prompt) => {
      hook.start(prompt, refusal(), 'session');
      return false;
    });
    await advance(5_000);

    expect(resend).toHaveBeenCalledTimes(1);
    expect(hook.state?.phase).toBe('waiting');
    expect(hook.state?.deadline).toBe(deadline);
    // Backoff level kept: the next check is 10 s out, not 5 s.
    expect(hook.state?.nextCheckAt).toBe(Date.now() + 10_000);
  });

  it('stops after the recovery window and offers a manual retry', async () => {
    mocks.listWorkers.mockResolvedValue([worker('think4', true)]);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    for (let elapsed = 0; elapsed < RECOVERY_WINDOW_MS; elapsed += 5_000) {
      await advance(5_000);
    }
    expect(hook.state?.phase).toBe('expired');
    expect(hook.state?.nextCheckAt).toBeNull();
    const calls = mocks.listWorkers.mock.calls.length;
    // 1 initial look + polls at 5, 15, 35, 75 s, then every 60 s to 10 min.
    expect(calls).toBeLessThanOrEqual(15);

    await advance(10 * 60_000);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(calls);
    expect(resend).not.toHaveBeenCalled();

    resend.mockResolvedValue(true);
    await act(async () => {
      await hook.retryNow();
    });
    expect(resend).toHaveBeenCalledWith('hello');
    expect(hook.state).toBeNull();
  });

  it('moves the workspace to the chosen server, then resends', async () => {
    mocks.listWorkers.mockResolvedValue([
      worker('think4', true),
      worker('think3'),
    ]);
    mocks.updateAffinity.mockResolvedValue({});
    resend.mockResolvedValue(true);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    await act(async () => {
      await hook.moveTo('think3');
    });
    expect(mocks.updateAffinity).toHaveBeenCalledWith('workspace', {
      run_on_coordinator: false,
      requested_worker_node_id: 'think3',
      restart_running: false,
      operation_id: null,
    });
    expect(resend).toHaveBeenCalledWith('hello');
    expect(hook.state).toBeNull();
  });

  it('keeps the notice and shows the reason when the move fails', async () => {
    mocks.listWorkers.mockResolvedValue([
      worker('think4', true),
      worker('think3'),
    ]);
    mocks.updateAffinity.mockRejectedValue(
      new Error('Stop running lifecycle scripts first')
    );
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();

    await act(async () => {
      await hook.moveTo('think3');
    });
    expect(resend).not.toHaveBeenCalled();
    expect(hook.state?.phase).toBe('suggesting');
    expect(hook.state?.error).toBe('Stop running lifecycle scripts first');
  });

  it('stops polling when dismissed or when the session changes', async () => {
    mocks.listWorkers.mockResolvedValue([worker('think4', true)]);
    render();
    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();
    act(() => hook.dismiss());
    expect(hook.state).toBeNull();
    await advance(60_000);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(1);

    act(() => {
      hook.start('hello', refusal(), 'session');
    });
    await flush();
    render('other-session');
    expect(hook.state).toBeNull();
    await advance(60_000);
    expect(mocks.listWorkers).toHaveBeenCalledTimes(2);
  });
});
