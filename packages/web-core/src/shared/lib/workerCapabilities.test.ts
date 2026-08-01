import { describe, expect, it } from 'vitest';
import {
  type WorkerNode,
  WorkerMountStatus,
  WorkerNodeStatus,
} from 'shared/types';

import {
  clusterAdvertisedProfiles,
  clusterSupportsExecutor,
} from './workerCapabilities';

function worker(overrides: Partial<WorkerNode> = {}): WorkerNode {
  return {
    id: 'worker-1',
    hostname: 'think3',
    status: WorkerNodeStatus.online,
    worker_version: '1',
    vibe_version: '1',
    capabilities: { executor_profiles: ['CLAUDE_CODE'] },
    resource_snapshot: { load_1m: 0.5, active_execution_count: 2 },
    labels: {},
    mount_status: WorkerMountStatus.healthy,
    mount_message: null,
    last_heartbeat_at: '2026-07-31T00:00:00Z',
    lease_expires_at: '2026-07-31T00:00:30Z',
    created_at: '2026-07-31T00:00:00Z',
    updated_at: '2026-07-31T00:00:00Z',
    ...overrides,
  };
}

describe('clusterAdvertisedProfiles', () => {
  it('collects profiles from workers that could accept work', () => {
    expect(
      clusterAdvertisedProfiles([
        worker({ capabilities: { executor_profiles: ['CLAUDE_CODE'] } }),
        worker({ id: 'w2', capabilities: { executor_profiles: ['CODEX'] } }),
      ])
    ).toEqual(['CLAUDE_CODE', 'CODEX']);
  });

  it('ignores workers that cannot accept work', () => {
    expect(
      clusterAdvertisedProfiles([
        worker({ status: WorkerNodeStatus.offline }),
        worker({
          id: 'w2',
          mount_status: WorkerMountStatus.read_only,
          capabilities: { executor_profiles: ['CODEX'] },
        }),
      ])
    ).toBeNull();
  });

  // Each of these would disable every agent if the helper returned an empty
  // array instead of "no opinion". `capabilities` arrives as `unknown`, so
  // none of these shapes is prevented by the type system.
  it.each([
    ['no workers at all', []],
    ['capabilities absent', [worker({ capabilities: undefined })]],
    ['capabilities not an object', [worker({ capabilities: 'codex' })]],
    ['capabilities null', [worker({ capabilities: null })]],
    ['profile list missing', [worker({ capabilities: { terminal: true } })]],
    [
      'profile list not an array',
      [worker({ capabilities: { executor_profiles: 'CODEX' } })],
    ],
    [
      'profile list holds non-strings',
      [worker({ capabilities: { executor_profiles: [1, null, {}] } })],
    ],
  ])('returns null when %s', (_label, workers) => {
    expect(clusterAdvertisedProfiles(workers as WorkerNode[])).toBeNull();
  });
});

describe('clusterSupportsExecutor', () => {
  it('permits everything when the cluster has no opinion', () => {
    expect(clusterSupportsExecutor(null, 'CODEX')).toBe(true);
  });

  it('gates on what the cluster advertises', () => {
    const advertised = ['CLAUDE_CODE'];
    expect(clusterSupportsExecutor(advertised, 'CLAUDE_CODE')).toBe(true);
    expect(clusterSupportsExecutor(advertised, 'CODEX')).toBe(false);
  });

  it('accepts a legacy lowercase advertisement', () => {
    // Workers registered against an older build keep non-canonical rows for
    // their whole uptime, so the UI sees them too.
    expect(clusterSupportsExecutor(['codex'], 'CODEX')).toBe(true);
    expect(clusterSupportsExecutor(['claude-code'], 'CLAUDE_CODE')).toBe(true);
  });

  it('resolves the CURSOR alias to the name the picker uses', () => {
    // BaseCodingAgent parses both, so a CURSOR row really can run
    // CURSOR_AGENT. Reporting it unsupported would withdraw a working agent.
    expect(clusterSupportsExecutor(['CURSOR'], 'CURSOR_AGENT')).toBe(true);
  });

  it('does not treat a pinned variant as general support', () => {
    // The whole point of comparing profiles rather than executor names: a
    // CODEX:PLAN-only cluster cannot serve a default Codex request.
    expect(clusterSupportsExecutor(['CODEX:PLAN'], 'CODEX')).toBe(false);
    expect(clusterSupportsExecutor(['CODEX:PLAN'], 'CODEX', 'DEFAULT')).toBe(
      false
    );
    expect(clusterSupportsExecutor(['CODEX:PLAN'], 'CODEX', 'PLAN')).toBe(true);
  });

  it('lets a bare advertisement serve any variant', () => {
    expect(clusterSupportsExecutor(['CODEX'], 'CODEX', 'PLAN')).toBe(true);
    expect(clusterSupportsExecutor(['CODEX'], 'CODEX', 'DEFAULT')).toBe(true);
  });

  it('does not match on a shared prefix', () => {
    expect(clusterSupportsExecutor(['CODEXFOO'], 'CODEX')).toBe(false);
  });
});
