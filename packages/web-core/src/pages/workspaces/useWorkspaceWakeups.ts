import { useQuery } from '@tanstack/react-query';
import { workspaceWakeupsApi } from '@/shared/lib/api';

export const workspaceWakeupsKey = (workspaceId?: string) =>
  ['workspace-wakeups', workspaceId] as const;

export function useWorkspaceWakeups(workspaceId?: string) {
  return useQuery({
    queryKey: workspaceWakeupsKey(workspaceId),
    queryFn: ({ signal }) => workspaceWakeupsApi.list(workspaceId!, signal),
    enabled: !!workspaceId,
    refetchInterval: 5000,
  });
}
