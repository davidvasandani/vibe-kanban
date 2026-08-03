import { describe, expect, it } from 'vitest';
import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';
import {
  getRecentProcessIdsToRetain,
  getUnloadedHistoricProcesses,
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
