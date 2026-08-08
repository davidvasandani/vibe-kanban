/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';

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

const initialData = () => ({ value: 'initial' });

function Fixture() {
  const result = useJsonPatchWsStream('/api/fixture', true, initialData);
  return (
    <div
      data-value={result.data?.value ?? 'missing'}
      data-connected={String(result.isConnected)}
      data-initialized={String(result.isInitialized)}
    />
  );
}

describe('useJsonPatchWsStream restart recovery', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    transport.open.mockReset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
  });

  it('keeps the last snapshot rendered while the same endpoint reconnects', async () => {
    const first = new FakeSocket();
    const second = new FakeSocket();
    transport.open
      .mockResolvedValueOnce(first as unknown as WebSocket)
      .mockResolvedValueOnce(second as unknown as WebSocket);

    await act(async () => root.render(<Fixture />));
    await act(async () => {});
    act(() => {
      first.onopen?.();
      first.onmessage?.({ data: JSON.stringify({ Ready: true }) });
      first.onmessage?.({
        data: JSON.stringify({
          JsonPatch: [{ op: 'replace', path: '/value', value: 'live' }],
        }),
      });
    });
    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'live'
    );

    act(() => first.onclose?.({ code: 1006, wasClean: false }));
    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'live'
    );
    expect(container.firstElementChild?.getAttribute('data-connected')).toBe(
      'false'
    );
    expect(container.firstElementChild?.getAttribute('data-initialized')).toBe(
      'true'
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_500);
    });
    await act(async () => {});
    expect(transport.open).toHaveBeenCalledTimes(2);
    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'live'
    );
  });
});
