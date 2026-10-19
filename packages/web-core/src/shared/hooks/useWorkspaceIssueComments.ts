import { useQuery } from '@tanstack/react-query';
import { issueCommentDeliveryApi } from '@/shared/lib/api';
import { withDeadline } from '@/shared/lib/issueCommentDelivery';
import { useHostId } from '@/shared/providers/HostIdProvider';

const POLL_MS = 10_000;

/** The linked issue's comments with delivery state, for a conversation. */
export function useWorkspaceIssueComments(workspaceId: string | undefined) {
  const hostId = useHostId();
  return useQuery({
    queryKey: ['workspaceIssueComments', hostId, workspaceId],
    queryFn: ({ signal }) =>
      withDeadline(
        issueCommentDeliveryApi.getWorkspaceIssueComments(workspaceId!, signal),
        signal
      ),
    enabled: Boolean(workspaceId),
    refetchInterval: POLL_MS,
    retry: 1,
  });
}
