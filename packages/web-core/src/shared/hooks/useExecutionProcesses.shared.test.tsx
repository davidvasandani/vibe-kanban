/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useExecutionProcesses } from './useExecutionProcesses';
import { resetSharedJsonPatchStreamsForTests } from '@/shared/lib/sharedJsonPatchStream';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const transport = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('@/shared/lib/localApiTransport', () => ({
  openLocalApiWebSocket: transport.open,
}));

class FakeSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((event: { code: number; wasClean: boolean }) => void) | null = null;
  close = vi.fn();
}

function Consumer({
  name,
  showSoftDeleted,
}: {
  name: string;
  showSoftDeleted?: boolean;
}) {
  const { executionProcesses, isLoading } = useExecutionProcesses(
    'session-1',
    showSoftDeleted === undefined ? undefined : { showSoftDeleted }
  );
  return (
    <div
      data-name={name}
      data-loading={String(isLoading)}
      data-ids={executionProcesses.map((p) => p.id).join(',')}
    />
  );
}

const processRow = (id: string, dropped: boolean) => ({
  id,
  session_id: 'session-1',
  dropped,
  created_at: `2026-09-30T00:00:0${id.length}Z`,
  run_reason: 'codingagent',
  status: 'completed',
});

describe('useExecutionProcesses socket sharing', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    transport.open.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    resetSharedJsonPatchStreamsForTests();
    container.remove();
  });

  it('opens one session socket for the layout, chat box and provider', async () => {
    const socket = new FakeSocket();
    transport.open.mockResolvedValue(socket);

    // The three consumers a workspace page mounts: the layout and the chat
    // box (plain) and the execution-processes provider (soft-deleted too).
    await act(async () =>
      root.render(
        <>
          <Consumer name="layout" />
          <Consumer name="chat-box" />
          <Consumer name="provider" showSoftDeleted />
        </>
      )
    );
    await act(async () => {});

    expect(transport.open).toHaveBeenCalledTimes(1);
    expect(transport.open.mock.calls[0][0]).toBe(
      '/api/execution-processes/stream/session/ws?session_id=session-1&show_soft_deleted=true'
    );

    act(() => {
      socket.onopen?.();
      socket.onmessage?.({
        data: JSON.stringify({
          JsonPatch: [
            {
              op: 'replace',
              path: '/execution_processes',
              value: {
                kept: processRow('kept', false),
                gone: processRow('gone', true),
              },
            },
          ],
        }),
      });
      socket.onmessage?.({ data: JSON.stringify({ Ready: true }) });
    });

    const ids = (name: string) =>
      container
        .querySelector(`[data-name="${name}"]`)
        ?.getAttribute('data-ids');
    expect(ids('layout')).toBe('kept');
    expect(ids('chat-box')).toBe('kept');
    expect(ids('provider')?.split(',').sort()).toEqual(['gone', 'kept']);
    expect(
      container
        .querySelector('[data-name="chat-box"]')
        ?.getAttribute('data-loading')
    ).toBe('false');
  });
});
