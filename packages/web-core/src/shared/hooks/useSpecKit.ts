import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  SpecKitArtifacts,
  SpecKitTaskStatus,
  SpecKitTasks,
  SpecKitToggleTaskRequest,
  SpecKitUpdateArtifactRequest,
} from 'shared/types';
import { speckitApi } from '@/shared/lib/api';

export const SPECKIT_STATUS_QUERY_KEY = (workspaceId: string) =>
  ['speckit', workspaceId, 'status'] as const;
export const SPECKIT_ARTIFACTS_QUERY_KEY = (workspaceId: string) =>
  ['speckit', workspaceId, 'artifacts'] as const;

/**
 * Fetch a workspace's SpecKit feature status (whether the viewer applies,
 * the resolved feature dir/host, per-stage artifact presence, parsed
 * tasks.md). Artifacts are written by the pipeline's execution agent, so a
 * short stale time keeps the rail current while an agent is working.
 */
export function useSpecKitStatus(workspaceId: string | null | undefined) {
  return useQuery<SpecKitTaskStatus>({
    queryKey: SPECKIT_STATUS_QUERY_KEY(workspaceId ?? ''),
    queryFn: () => speckitApi.getStatus(workspaceId!),
    enabled: !!workspaceId,
    staleTime: 15 * 1000,
  });
}

/** Fetch the feature's artifacts (spec/plan/tasks/research/…) off the worktree. */
export function useSpecKitArtifacts(
  workspaceId: string | null | undefined,
  enabled: boolean
) {
  return useQuery<SpecKitArtifacts>({
    queryKey: SPECKIT_ARTIFACTS_QUERY_KEY(workspaceId ?? ''),
    queryFn: () => speckitApi.getArtifacts(workspaceId!),
    enabled: !!workspaceId && enabled,
    staleTime: 15 * 1000,
  });
}

/** Save an edited artifact back to the worktree, then refresh both queries. */
export function useUpdateSpecKitArtifact(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: SpecKitUpdateArtifactRequest) =>
      speckitApi.updateArtifact(workspaceId, data),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: SPECKIT_ARTIFACTS_QUERY_KEY(workspaceId),
      });
      void queryClient.invalidateQueries({
        queryKey: SPECKIT_STATUS_QUERY_KEY(workspaceId),
      });
    },
  });
}

/** Toggle a tasks.md checkbox; the response is the freshly re-parsed tasks. */
export function useToggleSpecKitTask(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: SpecKitToggleTaskRequest) =>
      speckitApi.toggleTask(workspaceId, data),
    onSuccess: (tasks: SpecKitTasks) => {
      queryClient.setQueryData<SpecKitTaskStatus>(
        SPECKIT_STATUS_QUERY_KEY(workspaceId),
        (prev) => (prev ? { ...prev, tasks } : prev)
      );
      void queryClient.invalidateQueries({
        queryKey: SPECKIT_ARTIFACTS_QUERY_KEY(workspaceId),
      });
    },
  });
}
