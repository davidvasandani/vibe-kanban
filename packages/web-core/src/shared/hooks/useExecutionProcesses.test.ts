import { describe, expect, it } from 'vitest';
import { ExecutionProcessStatus } from 'shared/types';
import { hasRunningAttempt } from './useExecutionProcesses';

describe('hasRunningAttempt', () => {
  it('keeps active coding-agent executions cancellable', () => {
    expect(
      hasRunningAttempt([
        {
          run_reason: 'codingagent',
          status: ExecutionProcessStatus.running,
        },
      ])
    ).toBe(true);
  });

  it.each([
    ExecutionProcessStatus.completed,
    ExecutionProcessStatus.failed,
    ExecutionProcessStatus.killed,
    ExecutionProcessStatus.interrupted,
    ExecutionProcessStatus.indeterminate,
  ])('clears running UI for terminal status %s', (status) => {
    expect(
      hasRunningAttempt([
        {
          run_reason: 'codingagent',
          status,
        },
      ])
    ).toBe(false);
  });

  it('does not treat persistent helper processes as an active attempt', () => {
    expect(
      hasRunningAttempt([
        {
          run_reason: 'backgroundhelper',
          status: ExecutionProcessStatus.running,
        },
      ])
    ).toBe(false);
  });
});
