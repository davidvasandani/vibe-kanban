/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecutionProcess, PatchType, Workspace } from 'shared/types';
import type { ConversationTimelineSource } from '@/shared/hooks/useConversationHistory/types';
import { useConversationHistory } from './useConversationHistory';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const mocks = vi.hoisted(() => ({
  context: vi.fn(),
  load: vi.fn(),
  stream: vi.fn(),
}));
vi.mock('@/shared/hooks/useExecutionProcessesContext', () => ({
  useExecutionProcessesContext: mocks.context,
}));
vi.mock('@/shared/hooks/useConversationHistory/fetchHistoricEntries', () => ({
  fetchProcessLogSnapshot: vi.fn(),
  loadHistoricProcessEntries: mocks.load,
}));
vi.mock('@/shared/lib/streamJsonPatchEntries', () => ({
  streamJsonPatchEntries: mocks.stream,
}));

const entry = (content: string): PatchType => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: { type: 'assistant_message' },
    content,
    timestamp: null,
  },
});
const process = (
  id: string,
  status: ExecutionProcess['status'] = 'completed'
): ExecutionProcess =>
  ({
    id,
    status,
    session_id: 'session',
    run_reason: 'codingagent',
    dropped: false,
    created_at: `2026-11-23T00:00:${id.padStart(2, '0')}Z`,
    executor_action: { typ: { type: 'CodingAgentFollowUpRequest' } },
  }) as ExecutionProcess;

let source: ConversationTimelineSource;
let history: ReturnType<typeof useConversationHistory>;
const onTimelineUpdated = (next: ConversationTimelineSource) => {
  source = next;
};
function Consumer({ scope }: { scope: string }) {
  history = useConversationHistory({
    attempt: { id: 'workspace' } as Workspace,
    scopeKey: scope,
    onTimelineUpdated,
  });
  return null;
}

describe('conversation discovery after initial history', () => {
  let root: Root;
  let container: HTMLDivElement;
  const render = async (processes: ExecutionProcess[], scope = 'session') => {
    mocks.context.mockReturnValue({
      executionProcessesVisible: processes,
      isLoading: false,
      isConnected: true,
    });
    await act(async () => {
      root.render(<Consumer scope={scope} />);
    });
  };
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.load.mockImplementation(async (p: ExecutionProcess) => ({
      entries: [entry(p.id)],
      complete: true,
    }));
    mocks.stream.mockReturnValue({ close: vi.fn() });
    container = document.createElement('div');
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
  });

  it('loads a wakeup first discovered completed without a manual refresh', async () => {
    await render([process('1')]);
    await render([process('1'), process('2')]);
    expect(source.executionProcessState['2']?.entries[0].content).toMatchObject(
      { content: '2' }
    );
    expect(mocks.load).toHaveBeenCalledTimes(2);
    await render([process('1'), process('2')]);
    expect(mocks.load).toHaveBeenCalledTimes(2);
  });

  it('streams a running wakeup and reconciles its completed output', async () => {
    await render([process('1')]);
    await render([process('1'), process('2', 'running')]);
    const callbacks = mocks.stream.mock.calls[0][1];
    act(() => callbacks.onEntries([entry('live wakeup')]));
    expect(source.executionProcessState['2'].entries[0].content).toMatchObject({
      content: 'live wakeup',
    });
    await render([process('1'), process('2')]);
    expect(source.executionProcessState['2'].entries[0].content).toMatchObject({
      content: '2',
    });
  });

  it('keeps older unloaded history lazy when a completed wakeup arrives', async () => {
    mocks.load.mockImplementation(async (p: ExecutionProcess) => ({
      entries: Array.from({ length: 11 }, () => entry(p.id)),
      complete: true,
    }));
    const history = Array.from({ length: 10 }, (_, i) =>
      process(String(i + 1))
    );
    await render(history);
    expect(Object.keys(source.executionProcessState)).toEqual(['10']);
    mocks.load.mockClear();
    await render([...history, process('11')]);
    expect(Object.keys(source.executionProcessState)).toEqual(['10', '11']);
    expect(mocks.load).toHaveBeenCalledTimes(1);
    expect(mocks.load.mock.calls[0][0].id).toBe('11');
  });

  it('offers history recovery when a completed wakeup cannot be fetched', async () => {
    await render([process('1')]);
    expect(history.hasEarlierHistory).toBe(false);
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => {});
    mocks.load.mockRejectedValueOnce(new Error('temporary transport failure'));
    try {
      await render([process('1'), process('2')]);
      expect(history.hasEarlierHistory).toBe(true);
      await act(async () => {
        await history.loadEarlier();
      });
      expect(
        source.executionProcessState['2'].entries[0].content
      ).toMatchObject({ content: '2' });
      expect(history.hasEarlierHistory).toBe(false);
    } finally {
      warning.mockRestore();
    }
  });

  it('ignores delayed live entries from the prior session', async () => {
    await render([process('1', 'running')]);
    const callbacks = mocks.stream.mock.calls[0][1];
    await render([process('2')], 'other-session');
    act(() => callbacks.onEntries([entry('stale live output')]));
    expect(source.executionProcessState['1']).toBeUndefined();
    expect(source.executionProcessState['2'].entries[0].content).toMatchObject({
      content: '2',
    });
  });

  it('ignores non-conversation processes discovered after history loads', async () => {
    await render([process('1')]);
    await render([
      process('1'),
      { ...process('2'), run_reason: 'backgroundhelper' },
    ]);
    expect(source.executionProcessState['2']).toBeUndefined();
    expect(mocks.load).toHaveBeenCalledTimes(1);
  });

  for (const change of ['scope', 'removal'] as const) {
    it(`discards terminal output after ${change}`, async () => {
      await render([process('1')]);
      await render([process('1'), process('2', 'running')]);
      let resolve!: (value: unknown) => void;
      mocks.load.mockImplementationOnce(
        () =>
          new Promise((done) => {
            resolve = done;
          })
      );
      await render([process('1'), process('2')]);
      await render(
        [process('1')],
        change === 'scope' ? 'other-session' : 'session'
      );
      await act(async () => {
        resolve({ entries: [entry('late')], complete: true });
      });
      expect(source.executionProcessState['2']).toBeUndefined();
    });
  }
});
