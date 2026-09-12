import { describe, expect, it, vi } from 'vitest';
import { WorkspaceAffinityKind, WorkspacePlacementState } from 'shared/types';
import { getServerAffinityLabel } from './serverAffinityLabel';

const baseAffinity = {
  kind: WorkspaceAffinityKind.automatic,
  placement_state: WorkspacePlacementState.ready,
  worker_node_id: null,
  worker_hostname: null,
  requested_worker_node_id: null,
  requested_worker_hostname: null,
};

describe('getServerAffinityLabel', () => {
  it('prefers the assigned worker hostname', () => {
    expect(
      getServerAffinityLabel(
        {
          ...baseAffinity,
          worker_hostname: 'think4',
          requested_worker_hostname: 'think3',
        },
        () => 'Automatic'
      )
    ).toBe('think4');
  });

  it('falls back to the requested worker hostname', () => {
    expect(
      getServerAffinityLabel(
        { ...baseAffinity, requested_worker_hostname: 'think3' },
        () => 'Automatic'
      )
    ).toBe('think3');
  });

  it('uses the translated placement kind when no hostname is available', () => {
    const translateKind = vi.fn(() => 'Automatic');

    expect(getServerAffinityLabel(baseAffinity, translateKind)).toBe(
      'Automatic'
    );
    expect(translateKind).toHaveBeenCalledWith(WorkspaceAffinityKind.automatic);
  });

  it('renders no label while affinity summary data is absent', () => {
    expect(getServerAffinityLabel(undefined, () => 'Automatic')).toBeNull();
  });
});
