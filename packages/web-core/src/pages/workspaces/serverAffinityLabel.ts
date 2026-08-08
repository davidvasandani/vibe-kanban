import type { WorkspaceAffinitySummary } from 'shared/types';

export function getServerAffinityLabel(
  affinity: WorkspaceAffinitySummary | undefined,
  translateKind: (kind: WorkspaceAffinitySummary['kind']) => string
): string | null {
  if (!affinity) return null;

  return (
    affinity.worker_hostname ??
    affinity.requested_worker_hostname ??
    translateKind(affinity.kind)
  );
}
