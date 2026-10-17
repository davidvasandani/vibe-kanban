import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExecutionProcess, PatchType } from 'shared/types';
import {
  fetchProcessLogSnapshot,
  historicLogSnapshotPath,
  HistoryFetchError,
  loadHistoricProcessEntries,
} from './fetchHistoricEntries';

vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: vi.fn(),
}));

const agentProcess = (id = 'p1') =>
  ({
    id,
    executor_action: { typ: { type: 'CodingAgentFollowUpRequest' } },
  }) as unknown as ExecutionProcess;

const scriptProcess = (id = 's1') =>
  ({
    id,
    executor_action: { typ: { type: 'ScriptRequest' } },
  }) as unknown as ExecutionProcess;

const entry = (content: string): PatchType => ({ type: 'STDOUT', content });

const jsonResponse = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });

const ok = (entries: PatchType[], complete = true) =>
  jsonResponse({
    success: true,
    data: { entries, complete },
    error_data: null,
    message: null,
  });

describe('historicLogSnapshotPath', () => {
  it('uses normalized logs for agent turns and raw logs for scripts', () => {
    expect(historicLogSnapshotPath(agentProcess('a'))).toBe(
      '/api/execution-processes/a/normalized-logs'
    );
    expect(historicLogSnapshotPath(scriptProcess('s'))).toBe(
      '/api/execution-processes/s/raw-logs'
    );
  });
});

describe('fetchProcessLogSnapshot', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves the snapshot on success and passes an abort signal', async () => {
    const request = vi.fn().mockResolvedValue(ok([entry('hi')]));

    const snapshot = await fetchProcessLogSnapshot(agentProcess(), {
      deadlineMs: 1_000,
      request,
    });

    expect(snapshot).toEqual({ entries: [entry('hi')], complete: true });
    expect(request).toHaveBeenCalledWith(
      '/api/execution-processes/p1/normalized-logs',
      expect.objectContaining({ signal: expect.any(AbortSignal) })
    );
  });

  it('rejects at the deadline and aborts the request', async () => {
    let seenSignal: AbortSignal | undefined;
    const request = vi.fn((_path: string, init?: RequestInit) => {
      seenSignal = init?.signal ?? undefined;
      return new Promise<Response>(() => {});
    });

    const pending = fetchProcessLogSnapshot(agentProcess(), {
      deadlineMs: 30_000,
      request,
    });
    const outcome = pending.then(
      () => 'resolved',
      (error: HistoryFetchError) => error.reason
    );

    await vi.advanceTimersByTimeAsync(30_000);

    await expect(outcome).resolves.toBe('timeout');
    expect(seenSignal?.aborted).toBe(true);
  });

  it('rejects an error status instead of treating it as an empty turn', async () => {
    const request = vi
      .fn()
      .mockResolvedValue(jsonResponse({ message: 'boom' }, 500));

    await expect(
      fetchProcessLogSnapshot(agentProcess(), { deadlineMs: 1_000, request })
    ).rejects.toMatchObject({ reason: 'status', status: 500 });
  });

  it('rejects success: false and bodies that are not snapshots', async () => {
    const failed = vi
      .fn()
      .mockResolvedValue(
        jsonResponse({ success: false, data: null, message: 'nope' })
      );
    await expect(
      fetchProcessLogSnapshot(agentProcess(), {
        deadlineMs: 1_000,
        request: failed,
      })
    ).rejects.toMatchObject({ reason: 'api' });

    const malformed = vi
      .fn()
      .mockResolvedValue(new Response('<html>proxy error</html>'));
    await expect(
      fetchProcessLogSnapshot(agentProcess(), {
        deadlineMs: 1_000,
        request: malformed,
      })
    ).rejects.toMatchObject({ reason: 'parse' });
  });

  it('rejects when the caller aborts, and settles only once', async () => {
    let resolveRequest: (response: Response) => void = () => {};
    const request = vi.fn(
      () =>
        new Promise<Response>((resolve) => {
          resolveRequest = resolve;
        })
    );
    const caller = new AbortController();
    const settlements: string[] = [];

    const pending = fetchProcessLogSnapshot(agentProcess(), {
      deadlineMs: 30_000,
      signal: caller.signal,
      request,
    }).then(
      () => settlements.push('resolved'),
      (error: HistoryFetchError) => settlements.push(error.reason)
    );

    caller.abort();
    // A response that arrives after the abort, and the deadline after that,
    // must not settle the request a second time.
    resolveRequest(ok([entry('late')]));
    await vi.advanceTimersByTimeAsync(30_000);
    await pending;

    expect(settlements).toEqual(['aborted']);
  });

  it('rejects immediately for an already-aborted signal', async () => {
    const request = vi.fn();
    const caller = new AbortController();
    caller.abort();

    await expect(
      fetchProcessLogSnapshot(agentProcess(), {
        deadlineMs: 1_000,
        signal: caller.signal,
        request,
      })
    ).rejects.toMatchObject({ reason: 'aborted' });
    expect(request).not.toHaveBeenCalled();
  });

  it('rejects a transport failure', async () => {
    const request = vi.fn().mockRejectedValue(new TypeError('offline'));

    await expect(
      fetchProcessLogSnapshot(agentProcess(), { deadlineMs: 1_000, request })
    ).rejects.toMatchObject({ reason: 'network' });
  });
});

describe('loadHistoricProcessEntries', () => {
  it('returns a settled snapshot without opening a websocket', async () => {
    const streamFallback = vi.fn();
    const result = await loadHistoricProcessEntries(agentProcess(), {
      fetchSnapshot: vi
        .fn()
        .mockResolvedValue({ entries: [entry('done')], complete: true }),
      streamFallback,
    });

    expect(result).toEqual({ entries: [entry('done')], complete: true });
    expect(streamFallback).not.toHaveBeenCalled();
  });

  it('falls back to the live replay when the process finished during the load', async () => {
    // The process snapshot said "completed", but its log store was still
    // live when the GET arrived, so the server answered with a prefix.
    const streamFallback = vi
      .fn()
      .mockResolvedValue([entry('first'), entry('final answer')]);

    const result = await loadHistoricProcessEntries(agentProcess(), {
      fetchSnapshot: vi
        .fn()
        .mockResolvedValue({ entries: [entry('first')], complete: false }),
      streamFallback,
    });

    expect(streamFallback).toHaveBeenCalledTimes(1);
    expect(result).toEqual({
      entries: [entry('first'), entry('final answer')],
      complete: true,
    });
  });

  it('rejects when the fallback fails, never returning the prefix as final', async () => {
    await expect(
      loadHistoricProcessEntries(agentProcess(), {
        fetchSnapshot: vi
          .fn()
          .mockResolvedValue({ entries: [entry('prefix')], complete: false }),
        streamFallback: vi.fn().mockRejectedValue(new Error('closed')),
      })
    ).rejects.toThrow('closed');
  });
});
