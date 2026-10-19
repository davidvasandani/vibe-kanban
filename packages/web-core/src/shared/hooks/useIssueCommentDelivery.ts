import { useCallback, useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { IssueCommentDelivery } from 'shared/types';
import { issueCommentDeliveryApi } from '@/shared/lib/api';
import { withDeadline } from '@/shared/lib/issueCommentDelivery';
import { useHostId } from '@/shared/providers/HostIdProvider';

const DELIVERY_POLL_MS = 5_000;
const TARGETS_POLL_MS = 10_000;

/**
 * Delivery state of an issue's comments to linked agent sessions, from the
 * host that runs them. When the host cannot answer (remote-only app, signed
 * out, older backend) `available` is false and nothing is reported as
 * delivered.
 */
export function useIssueCommentDelivery(issueId: string | null | undefined) {
  const hostId = useHostId();
  const queryClient = useQueryClient();
  const enabled = Boolean(issueId);
  const targetsKey = useMemo(
    () => ['issueAgentTargets', hostId, issueId] as const,
    [hostId, issueId]
  );
  const deliveriesKey = useMemo(
    () => ['issueCommentDeliveries', hostId, issueId] as const,
    [hostId, issueId]
  );

  const targetsQuery = useQuery({
    queryKey: targetsKey,
    queryFn: ({ signal }) =>
      withDeadline(
        issueCommentDeliveryApi.getAgentTargets(issueId!, signal),
        signal
      ),
    enabled,
    refetchInterval: TARGETS_POLL_MS,
    retry: 1,
  });
  const deliveriesQuery = useQuery({
    queryKey: deliveriesKey,
    queryFn: ({ signal }) =>
      withDeadline(
        issueCommentDeliveryApi.getDeliveries(issueId!, signal),
        signal
      ),
    enabled,
    refetchInterval: DELIVERY_POLL_MS,
    retry: 1,
  });

  // Requests that failed before the backend stored anything, by comment id.
  const [requestErrors, setRequestErrors] = useState<Map<string, string>>(
    () => new Map()
  );

  const deliveriesByComment = useMemo(() => {
    const map = new Map<string, IssueCommentDelivery>();
    for (const delivery of deliveriesQuery.data ?? []) {
      map.set(delivery.comment_id, delivery);
    }
    return map;
  }, [deliveriesQuery.data]);

  const requestDelivery = useCallback(
    async (commentId: string, targetSessionId?: string | null) => {
      if (!issueId) return null;
      try {
        const delivery = await issueCommentDeliveryApi.request({
          comment_id: commentId,
          issue_id: issueId,
          target_session_id: targetSessionId ?? null,
        });
        setRequestErrors((current) => {
          if (!current.has(commentId)) return current;
          const next = new Map(current);
          next.delete(commentId);
          return next;
        });
        queryClient.setQueryData<IssueCommentDelivery[]>(
          deliveriesKey,
          (current) => [
            ...(current ?? []).filter((d) => d.comment_id !== commentId),
            delivery,
          ]
        );
        void queryClient.invalidateQueries({ queryKey: targetsKey });
        return delivery;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Delivery request failed';
        setRequestErrors((current) => new Map(current).set(commentId, message));
        return null;
      }
    },
    [issueId, queryClient, deliveriesKey, targetsKey]
  );

  return {
    available: targetsQuery.isSuccess,
    unavailableError: targetsQuery.isError ? targetsQuery.error : null,
    targets: targetsQuery.data ?? null,
    deliveriesByComment,
    requestErrors,
    requestDelivery,
  };
}
