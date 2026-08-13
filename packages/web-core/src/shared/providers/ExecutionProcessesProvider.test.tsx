/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecutionProcess } from 'shared/types';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import { ExecutionProcessesProvider } from './ExecutionProcessesProvider';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const executionHook = vi.hoisted(() => ({ use: vi.fn() }));
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
  dropped = false
): ExecutionProcess {
  return {
    id: `${runReason}-${status}-${dropped}`,
    run_reason: runReason,
    status,
    dropped,
  } as ExecutionProcess;
}

function Consumer() {
  const { isAttemptRunningVisible } = useExecutionProcessesContext();
  return <div data-running={String(isAttemptRunningVisible)} />;
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
});
