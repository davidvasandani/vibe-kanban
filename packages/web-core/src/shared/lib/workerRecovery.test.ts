import { describe, expect, it } from 'vitest';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type WorkerNode,
} from 'shared/types';
import { ApiError } from '@/shared/lib/api';
import {
  asWorkerUnavailable,
  isWorkerAccepting,
  rankAlternativeWorkers,
  recoveryBackoffMs,
} from './workerRecovery';

const NOW = Date.parse('2026-10-21T12:00:00Z');

function worker(
  id: string,
  snapshot: Record<string, unknown> = {},
  overrides: Partial<WorkerNode> = {}
): WorkerNode {
  return {
    id,
    hostname: id,
    status: WorkerNodeStatus.online,
    worker_version: '1',
    vibe_version: '1',
    capabilities: { executor_profiles: ['CLAUDE_CODE'] },
    resource_snapshot: {
      load_1m: 1,
      active_execution_count: 0,
      admission_refusing: false,
      ...snapshot,
    },
    labels: {},
    mount_status: WorkerMountStatus.healthy,
    mount_message: null,
    last_heartbeat_at: new Date(NOW).toISOString(),
    lease_expires_at: new Date(NOW + 30_000).toISOString(),
    created_at: new Date(NOW).toISOString(),
    updated_at: new Date(NOW).toISOString(),
    ...overrides,
  };
}

describe('asWorkerUnavailable', () => {
  const data = {
    worker_node_id: 'think4-id',
    worker: 'think4',
    reason: 'refusing',
    retryable: true,
  };

  it('reads the typed 503 payload', () => {
    expect(
      asWorkerUnavailable(new ApiError('refused', 503, undefined, data))
    ).toEqual(data);
  });

  it('ignores other statuses, untyped errors and unknown reasons', () => {
    expect(
      asWorkerUnavailable(new ApiError('boom', 500, undefined, data))
    ).toBeNull();
    expect(asWorkerUnavailable(new ApiError('refused', 503))).toBeNull();
    expect(asWorkerUnavailable(new Error('refused'))).toBeNull();
    expect(
      asWorkerUnavailable(
        new ApiError('refused', 503, undefined, { ...data, reason: 'new' })
      )
    ).toBeNull();
  });
});

describe('isWorkerAccepting', () => {
  it('requires eligibility and no admission refusal', () => {
    expect(isWorkerAccepting(worker('a'), 'CLAUDE_CODE', NOW)).toBe(true);
    expect(
      isWorkerAccepting(
        worker('a', { admission_refusing: true }),
        'CLAUDE_CODE',
        NOW
      )
    ).toBe(false);
    expect(
      isWorkerAccepting(
        worker('a', {}, { status: WorkerNodeStatus.draining }),
        'CLAUDE_CODE',
        NOW
      )
    ).toBe(false);
    expect(isWorkerAccepting(worker('a'), 'CODEX', NOW)).toBe(false);
  });
});

describe('rankAlternativeWorkers', () => {
  it('excludes the refusing server, refusing and ineligible ones, best first', () => {
    const workers = [
      worker('refused'),
      worker('busy', { load_1m: 4 }),
      worker('idle', { load_1m: 0.5 }),
      worker('tied', { load_1m: 0.5, active_execution_count: 2 }),
      worker('pressured', { admission_refusing: true, load_1m: 0 }),
      worker('expired', {}, { lease_expires_at: new Date(NOW).toISOString() }),
      worker('no-metrics', { load_1m: undefined }),
    ];

    expect(
      rankAlternativeWorkers(workers, 'refused', 'CLAUDE_CODE', NOW).map(
        (candidate) => candidate.id
      )
    ).toEqual(['idle', 'tied', 'busy', 'no-metrics']);
  });
});

describe('recoveryBackoffMs', () => {
  it('doubles from 5 s and caps at 60 s', () => {
    expect([0, 1, 2, 3, 4, 5, 12].map(recoveryBackoffMs)).toEqual([
      5_000, 10_000, 20_000, 40_000, 60_000, 60_000, 60_000,
    ]);
  });
});
