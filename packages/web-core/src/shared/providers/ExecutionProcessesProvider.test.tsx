/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecutionProcess } from 'shared/types';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import { ExecutionProcessesProvider } from './ExecutionProcessesProvider';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const executionHook = vi.hoisted(() => ({ use: vi.fn() }));
vi.mock('@/shared/lib/hmrContext', async () => {
  const { createContext } = await import('react');
  return {
    createHmrContext: <T,>(_key: string, value: T) => createContext(value),
  };
});
vi.mock('@/shared/hooks/useExecutionProcesses', async (importOriginal) => {
  const original =
    await importOriginal<
      typeof import('@/shared/hooks/useExecutionProcesses')
    >();
  return { ...original, useExecutionProcesses: executionHook.use };
});

function process(
  runReason: ExecutionProcess['run_reason'],
  status: ExecutionProcess['status'],
  dropped = false,
  overrides: Partial<ExecutionProcess> = {}
): ExecutionProcess {
  return {
    id: `${runReason}-${status}-${dropped}`,
    session_id: 'session',
    run_reason: runReason,
    status,
    dropped,
    created_at: '2026-09-16T00:00:00Z',
    ...overrides,
  } as ExecutionProcess;
}

let reconcileProcess:
  | ((executionProcess: ExecutionProcess) => void)
  | undefined;

function Consumer() {
  const {
    executionProcessesAll,
    isAttemptRunningVisible,
    reconcileExecutionProcess,
  } = useExecutionProcessesContext();
  reconcileProcess = reconcileExecutionProcess;
  return (
    <div
      data-running={String(isAttemptRunningVisible)}
      data-processes={executionProcessesAll
        .map((item) => `${item.id}:${item.status}`)
        .join(',')}
    />
  );
}

function setProcesses(processes: ExecutionProcess[]) {
  executionHook.use.mockReturnValue({
    executionProcesses: processes,
    executionProcessesById: Object.fromEntries(
      processes.map((item) => [item.id, item])
    ),
    isAttemptRunning: false,
    isLoading: false,
    isConnected: true,
    error: null,
  });
}

describe('ExecutionProcessesProvider composer activity boundary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    executionHook.use.mockReset();
    reconcileProcess = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  for (const runReason of [
    'codingagent',
    'setupscript',
    'cleanupscript',
    'archivescript',
  ] as const) {
    it(`exposes running ${runReason} as cancellable`, () => {
      setProcesses([process(runReason, 'running')]);
      act(() =>
        root.render(
          <ExecutionProcessesProvider sessionId="session">
            <Consumer />
          </ExecutionProcessesProvider>
        )
      );
      expect(container.firstElementChild?.getAttribute('data-running')).toBe(
        'true'
      );
    });
  }

  for (const status of [
    'completed',
    'failed',
    'killed',
    'interrupted',
    'indeterminate',
  ] as const) {
    it(`exposes ${status} as Send`, () => {
      setProcesses([process('codingagent', status)]);
      act(() =>
        root.render(
          <ExecutionProcessesProvider sessionId="session">
            <Consumer />
          </ExecutionProcessesProvider>
        )
      );
      expect(container.firstElementChild?.getAttribute('data-running')).toBe(
        'false'
      );
    });
  }

  it('does not expose a dropped running process as cancellable', () => {
    setProcesses([process('setupscript', 'running', true)]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    expect(container.firstElementChild?.getAttribute('data-running')).toBe(
      'false'
    );
  });

  it('projects a server-confirmed process before the stream reports it', () => {
    setProcesses([]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );

    act(() => {
      reconcileProcess?.(
        process('codingagent', 'running', false, { id: 'accepted' })
      );
    });

    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      'accepted:running'
    );
  });

  it('lets the stream supersede and later remove a reconciled process', () => {
    setProcesses([]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    act(() => {
      reconcileProcess?.(
        process('codingagent', 'running', false, { id: 'accepted' })
      );
    });

    setProcesses([
      process('codingagent', 'completed', false, { id: 'accepted' }),
    ]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      'accepted:completed'
    );

    setProcesses([]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      ''
    );
  });

  it('does not duplicate a process when the stream wins the race', () => {
    setProcesses([
      process('codingagent', 'running', false, { id: 'streamed' }),
    ]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    act(() => {
      reconcileProcess?.(
        process('codingagent', 'running', false, { id: 'streamed' })
      );
    });

    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      'streamed:running'
    );
  });

  it('rejects mismatched and stale-session process responses', () => {
    setProcesses([]);
    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    const staleReconcile = reconcileProcess;

    act(() => {
      reconcileProcess?.(
        process('codingagent', 'running', false, {
          id: 'mismatch',
          session_id: 'other',
        })
      );
    });
    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      ''
    );

    act(() =>
      root.render(
        <ExecutionProcessesProvider sessionId="next-session">
          <Consumer />
        </ExecutionProcessesProvider>
      )
    );
    act(() => {
      staleReconcile?.(
        process('codingagent', 'running', false, {
          id: 'stale',
          session_id: 'session',
        })
      );
    });
    expect(container.firstElementChild?.getAttribute('data-processes')).toBe(
      ''
    );
  });
});
