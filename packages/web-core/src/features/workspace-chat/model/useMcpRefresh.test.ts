/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { McpRefreshResult, McpRefreshStatus } from 'shared/types';
import { useMcpRefresh } from './useMcpRefresh';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    info: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
  },
}));

function refreshResult(
  status: McpRefreshStatus,
  generation = 1n
): McpRefreshResult {
  return {
    status,
    retryable: status === 'busy' || status === 'failed',
    generation,
    requested_at: '2026-08-04T10:00:00Z',
    last_successful_refresh_at:
      status === 'refreshed' ? '2026-08-04T10:00:02Z' : null,
    servers: [],
    error: null,
  };
}

type HookValue = ReturnType<typeof useMcpRefresh>;

let container: HTMLDivElement;
let root: Root;
let current: HookValue;
let workspaceId = 'workspace-1';
let sessionId = 'session-1';
const api = {
  refreshMcpTools: vi.fn(),
  getMcpRefreshStatus: vi.fn(),
};

function Harness() {
  current = useMcpRefresh(workspaceId, sessionId, {
    api,
    pollIntervalMs: 100,
  });
  return null;
}

async function renderHook() {
  await act(async () => {
    root.render(<Harness />);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  workspaceId = 'workspace-1';
  sessionId = 'session-1';
  api.refreshMcpTools.mockReset();
  api.getMcpRefreshStatus.mockReset();
  api.getMcpRefreshStatus.mockResolvedValue(null);
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('useMcpRefresh', () => {
  it('hydrates and polls an existing pending generation to completion', async () => {
    api.getMcpRefreshStatus
      .mockResolvedValueOnce(refreshResult('pending_next_turn'))
      .mockResolvedValueOnce(refreshResult('refreshed'));

    await renderHook();
    expect(current.result?.status).toBe('pending_next_turn');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(current.result?.status).toBe('refreshed');
    expect(api.getMcpRefreshStatus).toHaveBeenCalledTimes(2);
  });

  it('reconciles a duplicate busy response back to canonical pending state', async () => {
    api.refreshMcpTools.mockResolvedValue(refreshResult('busy'));
    api.getMcpRefreshStatus
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(refreshResult('pending_next_turn'))
      .mockResolvedValueOnce(refreshResult('refreshed'));

    await renderHook();
    await act(async () => {
      await current.refresh();
    });

    expect(current.result?.status).toBe('pending_next_turn');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(current.result?.status).toBe('refreshed');
  });

  it('ignores hydration from a session that is no longer selected', async () => {
    let resolveOldStatus: (value: McpRefreshResult | null) => void = () => {};
    api.getMcpRefreshStatus.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOldStatus = resolve;
        })
    );

    await renderHook();
    sessionId = 'session-2';
    api.getMcpRefreshStatus.mockResolvedValueOnce(null);
    await renderHook();

    await act(async () => {
      resolveOldStatus(refreshResult('pending_next_turn'));
    });

    expect(current.result).toBeNull();
  });
});
