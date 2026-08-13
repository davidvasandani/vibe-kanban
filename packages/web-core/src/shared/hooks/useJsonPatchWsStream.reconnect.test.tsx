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
      data-error={result.error ?? ''}
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

  it('replaces a stale running snapshot with the terminal reconnect snapshot', async () => {
    const first = new FakeSocket();
    const second = new FakeSocket();
    transport.open
      .mockResolvedValueOnce(first as unknown as WebSocket)
      .mockResolvedValueOnce(second as unknown as WebSocket);

    await act(async () => root.render(<Fixture />));
    await act(async () => {});
    act(() => {
      first.onopen?.();
      first.onmessage?.({
        data: JSON.stringify({
          JsonPatch: [{ op: 'replace', path: '/value', value: 'running' }],
        }),
      });
      first.onmessage?.({ data: JSON.stringify({ Ready: true }) });
    });
    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'running'
    );

    // The terminal event is missed while disconnected, so the last known
    // running state remains visible until the authoritative reconnect snapshot.
    act(() => first.onclose?.({ code: 1011, wasClean: false }));
    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'running'
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_500);
    });
    await act(async () => {});
    act(() => {
      second.onopen?.();
      second.onmessage?.({
        data: JSON.stringify({
          JsonPatch: [{ op: 'replace', path: '/value', value: 'interrupted' }],
        }),
      });
      second.onmessage?.({ data: JSON.stringify({ Ready: true }) });
    });

    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'interrupted'
    );
    expect(container.firstElementChild?.getAttribute('data-initialized')).toBe(
      'true'
    );
  });

  it('surfaces bounded initial connection failure despite allocated initial data', async () => {
    transport.open.mockRejectedValue(new Error('offline'));
    await act(async () => root.render(<Fixture />));

    for (let attempt = 0; attempt < 7; attempt += 1) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(8_000);
      });
      await act(async () => {});
    }

    expect(container.firstElementChild?.getAttribute('data-value')).toBe(
      'missing'
    );
    expect(container.firstElementChild?.getAttribute('data-initialized')).toBe(
      'false'
    );
    expect(container.firstElementChild?.getAttribute('data-error')).toBe(
      'Connection failed'
    );
  });

  it('does not reset backoff when sockets open but close before Ready', async () => {
    const sockets = Array.from({ length: 4 }, () => new FakeSocket());
    for (const socket of sockets) {
      transport.open.mockResolvedValueOnce(socket as unknown as WebSocket);
    }
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    await act(async () => root.render(<Fixture />));
    await act(async () => {});

    act(() => {
      sockets[0].onopen?.();
      sockets[0].onclose?.({ code: 1011, wasClean: false });
    });
    await act(async () => vi.advanceTimersByTimeAsync(1_999));
    expect(transport.open).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(1));
    await act(async () => {});

    act(() => {
      sockets[1].onopen?.();
      sockets[1].onclose?.({ code: 1011, wasClean: false });
    });
    await act(async () => vi.advanceTimersByTimeAsync(3_999));
    expect(transport.open).toHaveBeenCalledTimes(2);
    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(transport.open).toHaveBeenCalledTimes(3);
  });
});
