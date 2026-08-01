import { describe, expect, it } from 'vitest';
import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';
import { getUnloadedHistoricProcesses } from './conversation-history-paging';

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
