import { useQuery } from '@tanstack/react-query';
import type { Workspace } from 'shared/types';
import { workspacesApi } from '@/shared/lib/api';
import { getHostRequestScopeQueryKey } from '@/shared/lib/hostRequestScope';
import { useHostId } from '@/shared/providers/HostIdProvider';

export function useWorkspaceCreationProgress(workspace: Workspace) {
  const hostId = useHostId();
  const pending =
    workspace.creation_status === 'queued' ||
    workspace.creation_status === 'running';
  return useQuery({
    queryKey: [
      'workspaceCreationProgress',
      getHostRequestScopeQueryKey(hostId),
      workspace.id,
    ],
    queryFn: () => workspacesApi.getCreationProgress(workspace.id),
    enabled: workspace.creation_status !== 'ready',
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return pending && status !== 'ready' && status !== 'failed'
        ? 1000
        : false;
    },
    retry: false,
  });
}
