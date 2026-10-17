/* @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { streamJsonPatchEntries } from './streamJsonPatchEntries';

const transport = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('@/shared/lib/localApiTransport', () => ({
  openLocalApiWebSocket: transport.open,
}));

type Listener = (event: unknown) => void;

class FakeSocket {
  private listeners = new Map<string, Listener[]>();
  close = vi.fn(() => {
    this.emit('close', { code: 1000, wasClean: true });
  });

  addEventListener(type: string, listener: Listener) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  emit(type: string, event: unknown = {}) {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }

  message(payload: unknown) {
    this.emit('message', { data: JSON.stringify(payload) });
  }
}

const addEntry = (index: number, value: string) => ({
  JsonPatch: [{ op: 'add', path: `/entries/${index}`, value }],
});

async function open(opts: Parameters<typeof streamJsonPatchEntries>[1] = {}) {
  const socket = new FakeSocket();
  transport.open.mockResolvedValueOnce(socket as unknown as WebSocket);
  const onError = vi.fn();
  const onFinished = vi.fn();
  const controller = streamJsonPatchEntries<string>('/api/fixture', {
    onError,
    onFinished,
    ...opts,
  });
  // Let the async openLocalApiWebSocket resolve and listeners attach.
  await vi.advanceTimersByTimeAsync(0);
  return { socket, controller, onError, onFinished };
}

describe('streamJsonPatchEntries settlement', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    transport.open.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves once on finished and ignores the close that follows', async () => {
    const { socket, onError, onFinished } = await open();

    socket.message(addEntry(0, 'a'));
    socket.message({ finished: true });

    expect(onFinished).toHaveBeenCalledTimes(1);
    expect(onFinished).toHaveBeenCalledWith(['a']);
    expect(socket.close).toHaveBeenCalled();
    expect(onError).not.toHaveBeenCalled();
  });

  it('fails once when the socket closes cleanly without finished', async () => {
    const { socket, onError, onFinished } = await open();

    socket.message(addEntry(0, 'a'));
    socket.emit('close', { code: 1000, wasClean: true });

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onFinished).not.toHaveBeenCalled();
  });

  it('fails once on an error followed by a close', async () => {
    const { socket, onError } = await open();

    socket.emit('error', new Event('error'));
    socket.emit('close', { code: 1006, wasClean: false });

    expect(onError).toHaveBeenCalledTimes(1);
  });

  it('fails and closes the socket after the idle deadline', async () => {
    const { socket, onError } = await open({ idleTimeoutMs: 1_000 });

    await vi.advanceTimersByTimeAsync(999);
    expect(onError).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(onError).toHaveBeenCalledTimes(1);
    expect(socket.close).toHaveBeenCalled();
  });

  it('does not time out while messages keep arriving', async () => {
    const { socket, onError, onFinished } = await open({
      idleTimeoutMs: 1_000,
    });

    for (let i = 0; i < 5; i++) {
      await vi.advanceTimersByTimeAsync(800);
      socket.message(addEntry(i, `e${i}`));
    }
    socket.message({ finished: true });
    await vi.advanceTimersByTimeAsync(5_000);

    expect(onError).not.toHaveBeenCalled();
    expect(onFinished).toHaveBeenCalledTimes(1);
  });

  it('times out a socket that never finishes opening', async () => {
    transport.open.mockReturnValueOnce(new Promise(() => {}));
    const onError = vi.fn();
    streamJsonPatchEntries('/api/fixture', { onError, idleTimeoutMs: 1_000 });

    await vi.advanceTimersByTimeAsync(1_000);

    expect(onError).toHaveBeenCalledTimes(1);
  });

  it('reports nothing when the caller closes the stream', async () => {
    const { controller, onError, onFinished } = await open({
      idleTimeoutMs: 1_000,
    });

    controller.close();
    await vi.advanceTimersByTimeAsync(5_000);

    expect(onError).not.toHaveBeenCalled();
    expect(onFinished).not.toHaveBeenCalled();
  });

  it('never times out a live stream without an idle deadline', async () => {
    const { onError } = await open();

    await vi.advanceTimersByTimeAsync(10 * 60_000);

    expect(onError).not.toHaveBeenCalled();
  });
});
