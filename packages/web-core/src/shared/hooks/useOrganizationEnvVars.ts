import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { organizationsApi } from '@/shared/lib/api';
import { organizationKeys } from '@/shared/hooks/organizationKeys';
import type { OrganizationEnvVar } from 'shared/types';

interface UseOrganizationEnvVarsOptions {
  organizationId: string | null;
  enabled: boolean;
}

export function useOrganizationEnvVars({
  organizationId,
  enabled,
}: UseOrganizationEnvVarsOptions) {
  return useQuery<OrganizationEnvVar[]>({
    queryKey: organizationId
      ? organizationKeys.envVars(organizationId)
      : ['organizations', 'env-vars', 'disabled'],
    queryFn: () => organizationsApi.listEnvVars(organizationId!),
    enabled: enabled && Boolean(organizationId),
    staleTime: 60_000,
  });
}

interface MutationCallbacks {
  onSuccess?: () => void;
  onError?: (err: unknown) => void;
}

export function useOrganizationEnvVarMutations(
  orgId: string | null,
  callbacks?: MutationCallbacks
) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    if (!orgId) return;
    queryClient.invalidateQueries({
      queryKey: organizationKeys.envVars(orgId),
    });
  };

  const createEnvVar = useMutation({
    mutationFn: ({ name, value }: { name: string; value: string }) => {
      if (!orgId) throw new Error('Organization not selected');
      return organizationsApi.createEnvVar(orgId, { name, value });
    },
    onSuccess: () => {
      invalidate();
      callbacks?.onSuccess?.();
    },
    onError: callbacks?.onError,
  });

  const updateEnvVar = useMutation({
    mutationFn: ({ id, value }: { id: string; value: string }) => {
      if (!orgId) throw new Error('Organization not selected');
      return organizationsApi.updateEnvVar(orgId, id, { value });
    },
    onSuccess: () => {
      invalidate();
      callbacks?.onSuccess?.();
    },
    onError: callbacks?.onError,
  });

  const deleteEnvVar = useMutation({
    mutationFn: (id: string) => {
      if (!orgId) throw new Error('Organization not selected');
      return organizationsApi.deleteEnvVar(orgId, id);
    },
    onSuccess: () => {
      invalidate();
      callbacks?.onSuccess?.();
    },
    onError: callbacks?.onError,
  });

  return { createEnvVar, updateEnvVar, deleteEnvVar };
}
