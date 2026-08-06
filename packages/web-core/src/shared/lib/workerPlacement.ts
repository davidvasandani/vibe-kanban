import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type WorkerNode,
} from 'shared/types';

export function isWorkerEligible(
  worker: WorkerNode,
  now = Date.now(),
  requiredExecutorProfile?: string
): boolean {
  const baseEligible =
    worker.status === WorkerNodeStatus.online &&
    worker.mount_status === WorkerMountStatus.healthy &&
    !!worker.lease_expires_at &&
    new Date(worker.lease_expires_at).getTime() > now;
  if (!baseEligible || !requiredExecutorProfile) return baseEligible;

  const capabilities = worker.capabilities as {
    executor_profiles?: unknown;
  };
  const requested = requiredExecutorProfile.toLowerCase();
  return (
    Array.isArray(capabilities.executor_profiles) &&
    capabilities.executor_profiles.some(
      (advertised) =>
        typeof advertised === 'string' &&
        (advertised.toLowerCase() === requested ||
          (!advertised.includes(':') &&
            requested.startsWith(`${advertised.toLowerCase()}:`)))
    )
  );
}
