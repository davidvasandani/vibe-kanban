import { describe, expect, it } from 'vitest';
import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';
import {
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
