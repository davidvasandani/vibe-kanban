import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { jiraSyncApi } from '@/shared/lib/api';
import type {
  JiraSyncConfigResponse,
  JiraTestConnectionRequest,
  UpsertJiraSyncConfigRequest,
} from 'shared/remote-types';

export const jiraSyncKeys = {
  config: (projectId: string) => ['jira-sync', projectId, 'config'] as const,
};

interface UseJiraSyncConfigOptions {
  projectId: string | null;
  enabled?: boolean;
}

/** `data === null` means "no sync configured for this project". */
export function useJiraSyncConfig({
  projectId,
  enabled = true,
}: UseJiraSyncConfigOptions) {
  return useQuery<JiraSyncConfigResponse | null>({
    queryKey: projectId
      ? jiraSyncKeys.config(projectId)
      : ['jira-sync', 'disabled'],
    queryFn: () => jiraSyncApi.getConfig(projectId!),
    enabled: enabled && Boolean(projectId),
    // The reconciler stamps sync state server-side; refetch while the
    // settings screen is open so "last synced" stays fresh.
    refetchInterval: 15_000,
  });
}

export function useJiraSyncMutations(projectId: string | null) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    if (!projectId) return;
    queryClient.invalidateQueries({
      queryKey: jiraSyncKeys.config(projectId),
    });
  };

  const saveConfig = useMutation({
    mutationFn: (data: UpsertJiraSyncConfigRequest) => {
      if (!projectId) throw new Error('Project not selected');
      return jiraSyncApi.saveConfig(projectId, data);
    },
    onSuccess: invalidate,
  });

  const deleteConfig = useMutation({
    mutationFn: () => {
      if (!projectId) throw new Error('Project not selected');
      return jiraSyncApi.deleteConfig(projectId);
    },
    onSuccess: invalidate,
  });

  const testConnection = useMutation({
    mutationFn: (data: JiraTestConnectionRequest) => {
      if (!projectId) throw new Error('Project not selected');
      return jiraSyncApi.testConnection(projectId, data);
    },
  });

  const syncNow = useMutation({
    mutationFn: () => {
      if (!projectId) throw new Error('Project not selected');
      return jiraSyncApi.syncNow(projectId);
    },
    onSuccess: invalidate,
  });

  return { saveConfig, deleteConfig, testConnection, syncNow };
}
