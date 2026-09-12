import { describe, expect, it } from 'vitest';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type WorkerNode,
} from 'shared/types';
import { isWorkerEligible } from './workerPlacement';

function worker(overrides: Partial<WorkerNode> = {}): WorkerNode {
  return {
    id: 'worker-1',
    hostname: 'think1',
    status: WorkerNodeStatus.online,
    worker_version: '1',
    vibe_version: '1',
    capabilities: {},
    resource_snapshot: {},
    labels: {},
    mount_status: WorkerMountStatus.healthy,
    mount_message: null,
    last_heartbeat_at: '2026-08-05T00:00:00Z',
    lease_expires_at: '2026-08-05T00:01:00Z',
    created_at: '2026-08-05T00:00:00Z',
    updated_at: '2026-08-05T00:00:00Z',
    ...overrides,
  };
}

describe('isWorkerEligible', () => {
  const now = new Date('2026-08-05T00:00:30Z').getTime();

  it('accepts an online healthy worker with a live lease', () => {
    expect(isWorkerEligible(worker(), now)).toBe(true);
  });

  it('rejects offline, unhealthy, and expired workers', () => {
    expect(
      isWorkerEligible(worker({ status: WorkerNodeStatus.offline }), now)
    ).toBe(false);
    expect(
      isWorkerEligible(
        worker({ mount_status: WorkerMountStatus.read_only }),
        now
      )
    ).toBe(false);
    expect(
      isWorkerEligible(
        worker({ lease_expires_at: '2026-08-05T00:00:00Z' }),
        now
      )
    ).toBe(false);
  });

  it('matches executor capabilities using scheduler-compatible variant rules', () => {
    const candidate = worker({
      capabilities: { executor_profiles: ['codex', 'claude_code:PLAN'] },
    });
    expect(isWorkerEligible(candidate, now, 'CODEX:DEFAULT')).toBe(true);
    expect(isWorkerEligible(candidate, now, 'CLAUDE_CODE:PLAN')).toBe(true);
    expect(isWorkerEligible(candidate, now, 'CLAUDE_CODE:DEFAULT')).toBe(false);
    expect(isWorkerEligible(candidate, now, 'GEMINI:DEFAULT')).toBe(false);
  });
});
