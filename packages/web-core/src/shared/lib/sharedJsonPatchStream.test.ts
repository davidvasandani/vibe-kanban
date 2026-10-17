import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const transport = vi.hoisted(() => ({ open: vi.fn() }));
const host = vi.hoisted(() => ({ id: null as string | null }));

vi.mock('@/shared/lib/localApiTransport', () => ({
  openLocalApiWebSocket: transport.open,
}));
vi.mock('@/shared/providers/HostIdProvider', () => ({
  getCurrentHostId: () => host.id,
}));

import {
  acquireSharedJsonPatchStream,
  resetSharedJsonPatchStreamsForTests,
  STREAM_LINGER_MS,
} from './sharedJsonPatchStream';

class FakeSocket {
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: ((event: { code: number; wasClean: boolean }) => void) | null = null;
  close = vi.fn();
  send(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
}

const initialData = () => ({ value: 'initial' });
const acquire = (endpoint = '/api/fixture') =>
  acquireSharedJsonPatchStream(endpoint, { initialData });

async function flush() {
  await vi.advanceTimersByTimeAsync(0);
}

describe('SharedJsonPatchStream', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    transport.open.mockReset();
    host.id = null;
  });

  afterEach(() => {
    resetSharedJsonPatchStreamsForTests();
    vi.useRealTimers();
  });

  it('opens one socket for many subscribers and delivers to all of them', async () => {
    const socket = new FakeSocket();
    transport.open.mockResolvedValue(socket);

    const stream = acquire();
    expect(acquire()).toBe(stream);
    const a = vi.fn();
    const b = vi.fn();
    const c = vi.fn();
    stream.subscribe(a);
    acquire().subscribe(b);
    acquire().subscribe(c);
    await flush();

    expect(transport.open).toHaveBeenCalledTimes(1);
    socket.send({
      JsonPatch: [{ op: 'replace', path: '/value', value: 'live' }],
    });
    socket.send({ Ready: true });

    expect(stream.getSnapshot()).toMatchObject({
      data: { value: 'live' },
      isInitialized: true,
    });
    for (const listener of [a, b, c]) expect(listener).toHaveBeenCalled();
  });

  it('keeps the socket through a quick remount and closes it after the linger', async () => {
    const socket = new FakeSocket();
    transport.open.mockResolvedValue(socket);
    const stream = acquire();

    const unsubscribeFirst = stream.subscribe(() => {});
    await flush();
    unsubscribeFirst();
    await vi.advanceTimersByTimeAsync(STREAM_LINGER_MS - 1);
    const unsubscribeRemount = stream.subscribe(() => {});
    await vi.advanceTimersByTimeAsync(STREAM_LINGER_MS * 2);

    expect(transport.open).toHaveBeenCalledTimes(1);
    expect(socket.close).not.toHaveBeenCalled();

    unsubscribeRemount();
    await vi.advanceTimersByTimeAsync(STREAM_LINGER_MS);
    expect(socket.close).toHaveBeenCalledTimes(1);
    expect(stream.getSnapshot().data).toBeUndefined();
  });

  it('reconnects an unexpected close while keeping the last snapshot', async () => {
    const first = new FakeSocket();
    const second = new FakeSocket();
    transport.open.mockResolvedValueOnce(first).mockResolvedValueOnce(second);
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    const stream = acquire();
    stream.subscribe(() => {});
    await flush();
    first.send({ JsonPatch: [{ op: 'replace', path: '/value', value: 'a' }] });
    first.send({ Ready: true });

    first.onclose?.({ code: 1006, wasClean: false });
    expect(stream.getSnapshot()).toMatchObject({
      data: { value: 'a' },
      isConnected: false,
      isInitialized: true,
    });

    // Attempt 1 after Ready reset the count: 2 s with the jitter centred.
    await vi.advanceTimersByTimeAsync(1_999);
    expect(transport.open).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(transport.open).toHaveBeenCalledTimes(2);
  });

  it('does not reconnect after finished or a clean close', async () => {
    const finished = new FakeSocket();
    const clean = new FakeSocket();
    transport.open.mockResolvedValueOnce(finished).mockResolvedValueOnce(clean);

    acquire('/api/finished').subscribe(() => {});
    acquire('/api/clean').subscribe(() => {});
    await flush();
    finished.send({ finished: true });
    finished.onclose?.({ code: 1000, wasClean: true });
    clean.onclose?.({ code: 1000, wasClean: true });
    await vi.advanceTimersByTimeAsync(20_000);

    expect(transport.open).toHaveBeenCalledTimes(2);
  });

  it('keeps streams for different hosts apart', async () => {
    transport.open.mockImplementation(async () => new FakeSocket());
    host.id = 'host-a';
    const onA = acquire('/api/approvals/stream/ws');
    host.id = 'host-b';
    const onB = acquire('/api/approvals/stream/ws');

    expect(onB).not.toBe(onA);
  });

  it('reconnects to the host it was created for, not the current one', async () => {
    const first = new FakeSocket();
    transport.open
      .mockResolvedValueOnce(first)
      .mockResolvedValueOnce(new FakeSocket());
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    host.id = 'host-a';
    const onA = acquire('/api/approvals/stream/ws');
    onA.subscribe(() => {});
    await flush();

    // The route moves to host B while A's stream is still alive; then A's
    // socket drops and reconnects.
    host.id = 'host-b';
    first.onclose?.({ code: 1006, wasClean: false });
    await vi.advanceTimersByTimeAsync(8_000);

    expect(transport.open).toHaveBeenCalledTimes(2);
    for (const [, options] of transport.open.mock.calls) {
      expect(options).toMatchObject({
        hostScope: 'explicit',
        hostId: 'host-a',
        relayHostId: 'host-a',
      });
    }
  });

  it('keeps consumers that rewrite patches on a private stream', () => {
    const shared = acquire();
    const rewriting = acquireSharedJsonPatchStream('/api/fixture', {
      initialData,
      deduplicatePatches: (patches) => patches,
    });

    expect(rewriting).not.toBe(shared);
    expect(acquire()).toBe(shared);
  });
});
