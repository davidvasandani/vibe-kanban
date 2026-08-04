import { describe, expect, it } from 'vitest';
import {
  AUTOMATIC_PLACEMENT,
  COORDINATOR_PLACEMENT,
  serializeWorkspacePlacement,
} from './workspacePlacement';

describe('serializeWorkspacePlacement', () => {
  it('keeps automatic placement distinct from coordinator placement', () => {
    expect(serializeWorkspacePlacement(AUTOMATIC_PLACEMENT)).toEqual({
      run_on_coordinator: false,
      requested_worker_node_id: null,
    });
    expect(serializeWorkspacePlacement(COORDINATOR_PLACEMENT)).toEqual({
      run_on_coordinator: true,
      requested_worker_node_id: null,
    });
  });

  it('serializes a worker selection as an explicit worker override', () => {
    expect(serializeWorkspacePlacement('worker-id')).toEqual({
      run_on_coordinator: false,
      requested_worker_node_id: 'worker-id',
    });
  });
});
