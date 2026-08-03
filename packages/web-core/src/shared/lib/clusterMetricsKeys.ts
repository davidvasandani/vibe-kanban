import { getHostRequestScopeQueryKey } from '@/shared/lib/hostRequestScope';

/**
 * React Query keys for the cluster metrics snapshot.
 *
 * **Every key carries the host scope.** `/api/cluster/metrics` describes the
 * cluster of whichever host the request was routed to, so an unscoped key
 * would file host B's snapshot under host A's cache entry and render one
 * machine's readings labelled with another's hostname — the single worst
 * failure this panel can have.
 *
 * `undefined` (the ambient "current host") and `null` (explicitly local) are
 * distinct scopes and get distinct keys; see `getHostRequestScopeQueryKey`.
 */
export const clusterMetricsKeys = {
  all: ['clusterMetrics'] as const,
  scope: (hostId?: string | null) =>
    [...clusterMetricsKeys.all, getHostRequestScopeQueryKey(hostId)] as const,
  snapshot: (hostId?: string | null) =>
    [...clusterMetricsKeys.scope(hostId), 'snapshot'] as const,
};
