import { describe, expect, it } from 'vitest';
import {
  ExecutionProcessStatus,
  type ExecutionProcess,
  type ExecutionProcessRunReason,
} from 'shared/types';
import { derivePollersHeaderStatus, selectPollers } from './pollers';

function process(
  overrides: {
    id?: string;
    runReason?: ExecutionProcessRunReason;
    status?: ExecutionProcessStatus;
    poller?: { command: string; interval_secs: number } | null;
    actionType?: 'ScriptRequest' | 'CodingAgentInitialRequest';
  } = {}
): ExecutionProcess {
  const {
    id = 'p1',
    runReason = 'backgroundhelper',
    status = ExecutionProcessStatus.running,
    poller = { command: 'git fetch', interval_secs: 60 },
    actionType = 'ScriptRequest',
  } = overrides;

  return {
    id,
    session_id: 'session-1',
    run_reason: runReason,
    executor_action: {
      typ:
        actionType === 'ScriptRequest'
          ? ({
              type: 'ScriptRequest',
              script: 'generated',
              language: 'Bash',
              context: 'BackgroundHelper',
              working_dir: null,
              poller,
            } as never)
          : ({ type: 'CodingAgentInitialRequest' } as never),
      next_action: null,
    },
    status,
    exit_code: null,
    pgid: null,
    dropped: false,
    started_at: '2026-08-31T05:00:00Z',
    completed_at: null,
    created_at: '2026-08-31T05:00:00Z',
    updated_at: '2026-08-31T05:00:00Z',
  } as ExecutionProcess;
}

describe('selectPollers', () => {
  it('projects a poller off the streamed execution process', () => {
    expect(selectPollers([process()])).toEqual([
      {
        id: 'p1',
        command: 'git fetch',
        intervalSecs: 60,
        status: ExecutionProcessStatus.running,
        startedAt: '2026-08-31T05:00:00Z',
      },
    ]);
  });

  it('excludes a plain background helper, which carries no poller spec', () => {
    expect(selectPollers([process({ poller: null })])).toEqual([]);
  });

  it('excludes other run reasons even when they are script requests', () => {
    expect(selectPollers([process({ runReason: 'devserver' })])).toEqual([]);
  });

  it('excludes non-script actions without throwing', () => {
    expect(
      selectPollers([process({ actionType: 'CodingAgentInitialRequest' })])
    ).toEqual([]);
  });
});

describe('derivePollersHeaderStatus', () => {
  it('returns null when there is nothing decisive to report', () => {
    expect(derivePollersHeaderStatus([])).toBeNull();
  });

  it('returns null when every poller is in a quiet terminal state', () => {
    const status = derivePollersHeaderStatus(
      selectPollers([process({ status: ExecutionProcessStatus.killed })])
    );
    expect(status).toBeNull();
  });

  it('reports the running count', () => {
    const status = derivePollersHeaderStatus(
      selectPollers([process({ id: 'a' }), process({ id: 'b' })])
    );
    expect(status?.visibleText).toBe('2');
    expect(status?.accessibleText).toBe('2 pollers running');
    expect(status?.hasFailure).toBe(false);
  });

  it('surfaces a failure distinctly from the running count', () => {
    const status = derivePollersHeaderStatus(
      selectPollers([
        process({ id: 'a' }),
        process({ id: 'b', status: ExecutionProcessStatus.failed }),
      ])
    );
    expect(status?.visibleText).toBe('1 · 1 failed');
    expect(status?.hasFailure).toBe(true);
  });

  it('still reports a failure when nothing is running', () => {
    // The case a bare running count would render identically to "no pollers at
    // all" — which is the state most worth noticing.
    const status = derivePollersHeaderStatus(
      selectPollers([process({ status: ExecutionProcessStatus.failed })])
    );
    expect(status?.visibleText).toBe('1 failed');
    expect(status?.hasFailure).toBe(true);
  });
});
