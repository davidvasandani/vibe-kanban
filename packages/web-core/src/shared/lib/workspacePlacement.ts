import type { CreateAndStartWorkspaceRequest } from 'shared/types';

export const AUTOMATIC_PLACEMENT = 'automatic';
export const COORDINATOR_PLACEMENT = 'coordinator';

type PlacementFields = Pick<
  CreateAndStartWorkspaceRequest,
  'run_on_coordinator' | 'requested_worker_node_id'
>;

export function serializeWorkspacePlacement(
  selection: string
): PlacementFields {
  if (selection === COORDINATOR_PLACEMENT) {
    return {
      run_on_coordinator: true,
      requested_worker_node_id: null,
    };
  }

  return {
    run_on_coordinator: false,
    requested_worker_node_id:
      selection === AUTOMATIC_PLACEMENT ? null : selection,
  };
}
