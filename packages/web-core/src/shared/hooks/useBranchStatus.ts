import { useQuery } from '@tanstack/react-query';
import { workspacesApi } from '@/shared/lib/api';

// Each tick shells out to git several times per repo in the workspace
// (head info, rebase/conflict checks, status, ahead/behind). Keep this coarse
// enough that idle tabs don't turn into a steady stream of git subprocesses.
const BRANCH_STATUS_POLL_INTERVAL_MS = 15_000;

export function useBranchStatus(workspaceId?: string) {
  return useQuery({
    queryKey: ['branchStatus', workspaceId],
    queryFn: () => workspacesApi.getBranchStatus(workspaceId!),
    enabled: !!workspaceId,
    refetchInterval: BRANCH_STATUS_POLL_INTERVAL_MS,
    // Don't poll a backgrounded tab; it refetches on refocus instead.
    refetchIntervalInBackground: false,
  });
}
