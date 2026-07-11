import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { slackApi } from '@/shared/lib/api';
import type {
  SlackConfigResponse,
  UpsertSlackConfigRequest,
} from 'shared/remote-types';

export const slackIntegrationKeys = {
  config: (organizationId: string) =>
    ['slack-integration', organizationId, 'config'] as const,
};

interface UseSlackConfigOptions {
  organizationId: string | null;
  enabled?: boolean;
}

/** `data === null` means "no Slack workspace connected to this org". */
export function useSlackConfig({
  organizationId,
  enabled = true,
}: UseSlackConfigOptions) {
  return useQuery<SlackConfigResponse | null>({
    queryKey: organizationId
      ? slackIntegrationKeys.config(organizationId)
      : ['slack-integration', 'disabled'],
    queryFn: () => slackApi.getConfig(organizationId!),
    enabled: enabled && Boolean(organizationId),
  });
}

export function useSlackMutations(organizationId: string | null) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    if (!organizationId) return;
    queryClient.invalidateQueries({
      queryKey: slackIntegrationKeys.config(organizationId),
    });
  };

  const saveConfig = useMutation({
    mutationFn: (data: UpsertSlackConfigRequest) => {
      if (!organizationId) throw new Error('Organization not selected');
      return slackApi.saveConfig(organizationId, data);
    },
    onSuccess: invalidate,
  });

  const deleteConfig = useMutation({
    mutationFn: () => {
      if (!organizationId) throw new Error('Organization not selected');
      return slackApi.deleteConfig(organizationId);
    },
    onSuccess: invalidate,
  });

  const testConnection = useMutation({
    mutationFn: () => {
      if (!organizationId) throw new Error('Organization not selected');
      return slackApi.testConnection(organizationId);
    },
  });

  return { saveConfig, deleteConfig, testConnection };
}
