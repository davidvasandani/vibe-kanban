import { describe, expect, it, vi } from 'vitest';
import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';
import {
  createSettledEntriesCache,
  getRecentProcessIdsToRetain,
  getUnloadedHistoricProcesses,
  loadProcessesInOrder,
} from './conversation-history-paging';

function process(id: string, status: ExecutionProcessStatus): ExecutionProcess {
  return { id, status } as ExecutionProcess;
}

describe('getUnloadedHistoricProcesses', () => {
  it('returns only unloaded completed processes newest-first', () => {
    const processes = [
      process('oldest', ExecutionProcessStatus.completed),
      process('loaded', ExecutionProcessStatus.completed),
      process('newest', ExecutionProcessStatus.failed),
      process('live', ExecutionProcessStatus.running),
    ];

    expect(
      getUnloadedHistoricProcesses(processes, new Set(['loaded'])).map(
        ({ id }) => id
      )
    ).toEqual(['newest', 'oldest']);
  });

  it('returns an empty list when no earlier completed process remains', () => {
    const processes = [
      process('loaded', ExecutionProcessStatus.completed),
      process('live', ExecutionProcessStatus.running),
    ];

    expect(
      getUnloadedHistoricProcesses(processes, new Set(['loaded']))
    ).toEqual([]);
  });
});

describe('getRecentProcessIdsToRetain', () => {
  it('retains the newest loaded history window and every running process', () => {
    const processes = [
      process('oldest', ExecutionProcessStatus.completed),
      process('middle', ExecutionProcessStatus.completed),
      process('newest', ExecutionProcessStatus.completed),
      process('live', ExecutionProcessStatus.running),
    ];
    const entryCounts = new Map([
      ['oldest', 20],
      ['middle', 6],
      ['newest', 5],
      ['live', 100],
    ]);

    expect(getRecentProcessIdsToRetain(processes, entryCounts, 10, 20)).toEqual(
      new Set(['live', 'newest', 'middle'])
    );
  });

  it('ignores processes that are not currently loaded', () => {
    const processes = [
      process('unloaded', ExecutionProcessStatus.completed),
      process('loaded', ExecutionProcessStatus.completed),
    ];

    expect(
      getRecentProcessIdsToRetain(processes, new Map([['loaded', 12]]), 10, 20)
    ).toEqual(new Set(['loaded']));
  });

  it('caps retained empty historic processes independently of entry count', () => {
    const processes = [
      process('oldest', ExecutionProcessStatus.completed),
      process('middle', ExecutionProcessStatus.completed),
      process('newest', ExecutionProcessStatus.completed),
    ];
    const entryCounts = new Map([
      ['oldest', 0],
      ['middle', 0],
      ['newest', 0],
    ]);

    expect(getRecentProcessIdsToRetain(processes, entryCounts, 10, 2)).toEqual(
      new Set(['newest', 'middle'])
    );
  });
});

describe('loadProcessesInOrder', () => {
  const completed = (id: string) =>
    process(id, ExecutionProcessStatus.completed);

  /** A fetcher that records overlap and can be resolved out of request order. */
  function trackingFetcher(delays: Record<string, number>) {
    let inFlight = 0;
    let maxInFlight = 0;
    const started: string[] = [];

    const fetchEntries = async (p: ExecutionProcess) => {
      started.push(p.id);
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await new Promise((resolve) => setTimeout(resolve, delays[p.id] ?? 0));
      inFlight -= 1;
      return [`${p.id}-entry`];
    };

    return {
      fetchEntries,
      started,
      get maxInFlight() {
        return maxInFlight;
      },
    };
  }

  it('overlaps requests instead of awaiting them one at a time', async () => {
    const processes = ['a', 'b', 'c', 'd'].map(completed);
    const tracker = trackingFetcher({ a: 20, b: 20, c: 20, d: 20 });

    await loadProcessesInOrder(processes, tracker.fetchEntries, () => false, 4);

    expect(tracker.maxInFlight).toBe(4);
  });

  it('never exceeds the requested concurrency', async () => {
    const processes = ['a', 'b', 'c', 'd', 'e'].map(completed);
    const tracker = trackingFetcher({ a: 10, b: 10, c: 10, d: 10, e: 10 });

    await loadProcessesInOrder(processes, tracker.fetchEntries, () => false, 2);

    expect(tracker.maxInFlight).toBe(2);
  });

  /**
   * The window is "the newest processes until the threshold is crossed", so
   * ordering must come from the request sequence. Resolving the first request
   * last is the case that would expose an implementation that appended on
   * completion.
   */
  it('commits results in request order, not completion order', async () => {
    const processes = ['first', 'second', 'third'].map(completed);
    const tracker = trackingFetcher({ first: 30, second: 10, third: 0 });

    const { loaded } = await loadProcessesInOrder(
      processes,
      tracker.fetchEntries,
      () => false,
      3
    );

    expect(loaded.map(({ process: p }) => p.id)).toEqual([
      'first',
      'second',
      'third',
    ]);
  });

  it('stops where a serial fetch would, discarding surplus in-flight results', async () => {
    const processes = ['a', 'b', 'c', 'd'].map(completed);
    const tracker = trackingFetcher({});

    const { loaded } = await loadProcessesInOrder(
      processes,
      tracker.fetchEntries,
      (soFar) => soFar.length >= 2,
      4
    );

    // Concurrency decides how much is fetched, never how much is shown.
    expect(loaded.map(({ process: p }) => p.id)).toEqual(['a', 'b']);
    expect(tracker.started).toEqual(['a', 'b', 'c', 'd']);
  });

  it('skips a failed process and keeps the rest reachable', async () => {
    const processes = ['ok', 'broken', 'alsoOk'].map(completed);

    const { loaded, failedProcessCount } = await loadProcessesInOrder(
      processes,
      async (p: ExecutionProcess) => {
        if (p.id === 'broken') throw new Error('unreadable');
        return [`${p.id}-entry`];
      },
      () => false,
      3
    );

    expect(loaded.map(({ process: p }) => p.id)).toEqual(['ok', 'alsoOk']);
    expect(failedProcessCount).toBe(1);
  });
});

describe('createSettledEntriesCache', () => {
  const completed = (id: string) =>
    process(id, ExecutionProcessStatus.completed);

  it('requests each completed turn once across the initial window and load earlier', async () => {
    // Six completed turns, newest last, as the session snapshot lists them.
    const processes = ['t1', 't2', 't3', 't4', 't5', 't6'].map(completed);
    const cache = createSettledEntriesCache<string>();
    const requests: string[] = [];
    const fetchEntries = (p: ExecutionProcess) =>
      cache.get(p, async (target) => {
        requests.push(target.id);
        // The newest turn alone crosses the initial threshold.
        return {
          entries: target.id === 't6' ? Array(12).fill('e') : ['e'],
          complete: true,
        };
      });

    // Initial window: one concurrent slice of five, only the newest is kept.
    const initial = await loadProcessesInOrder(
      [...processes].reverse(),
      fetchEntries,
      (soFar) => soFar.flatMap(({ entries }) => entries).length > 10,
      5
    );
    expect(initial.loaded.map(({ process: p }) => p.id)).toEqual(['t6']);

    // The top sentinel then asks for earlier history.
    const earlier = await loadProcessesInOrder(
      getUnloadedHistoricProcesses(processes, new Set(['t6'])),
      fetchEntries,
      () => false,
      5
    );
    expect(earlier.loaded.map(({ process: p }) => p.id)).toEqual([
      't5',
      't4',
      't3',
      't2',
      't1',
    ]);

    expect([...requests].sort()).toEqual(['t1', 't2', 't3', 't4', 't5', 't6']);
  });

  it('joins a request already in flight for the same turn', async () => {
    const cache = createSettledEntriesCache<string>();
    let release: () => void = () => {};
    const load = vi.fn(
      () =>
        new Promise<{ entries: string[]; complete: boolean }>((resolve) => {
          release = () => resolve({ entries: ['a'], complete: true });
        })
    );

    const first = cache.get(completed('t1'), load);
    const second = cache.get(completed('t1'), load);
    release();

    await expect(Promise.all([first, second])).resolves.toEqual([['a'], ['a']]);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it('does not keep failures or incomplete results, so a retry refetches', async () => {
    const cache = createSettledEntriesCache<string>();
    const failing = vi.fn().mockRejectedValue(new Error('timeout'));
    await expect(cache.get(completed('t1'), failing)).rejects.toThrow(
      'timeout'
    );

    const partial = vi
      .fn()
      .mockResolvedValue({ entries: ['prefix'], complete: false });
    await expect(cache.get(completed('t1'), partial)).resolves.toEqual([
      'prefix',
    ]);

    const settled = vi
      .fn()
      .mockResolvedValue({ entries: ['full'], complete: true });
    await expect(cache.get(completed('t1'), settled)).resolves.toEqual([
      'full',
    ]);
    await expect(cache.get(completed('t1'), settled)).resolves.toEqual([
      'full',
    ]);
    expect(settled).toHaveBeenCalledTimes(1);
  });
});
