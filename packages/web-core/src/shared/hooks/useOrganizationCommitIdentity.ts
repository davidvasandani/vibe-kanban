import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { organizationsApi } from '@/shared/lib/api';
import { organizationKeys } from '@/shared/hooks/organizationKeys';
import type {
  OrganizationCommitIdentity,
  UpsertOrganizationCommitIdentityRequest,
} from 'shared/types';

export function useOrganizationCommitIdentity(organizationId: string | null) {
  return useQuery<OrganizationCommitIdentity | null>({
    queryKey: organizationId
      ? organizationKeys.commitIdentity(organizationId)
      : ['organizations', 'commit-identity', 'disabled'],
    queryFn: () => organizationsApi.getCommitIdentity(organizationId!),
    enabled: Boolean(organizationId),
    staleTime: 60_000,
  });
}

interface MutationCallbacks {
  onError?: (err: unknown) => void;
}

export function useOrganizationCommitIdentityMutations(
  orgId: string | null,
  callbacks?: MutationCallbacks
) {
  const queryClient = useQueryClient();
  const invalidate = () => {
    if (!orgId) return;
    queryClient.invalidateQueries({
      queryKey: organizationKeys.commitIdentity(orgId),
    });
  };

  const saveCommitIdentity = useMutation({
    mutationFn: (data: UpsertOrganizationCommitIdentityRequest) => {
      if (!orgId) throw new Error('Organization not selected');
      return organizationsApi.upsertCommitIdentity(orgId, data);
    },
    onSuccess: invalidate,
    onError: callbacks?.onError,
  });

  const removeCommitIdentity = useMutation({
    mutationFn: () => {
      if (!orgId) throw new Error('Organization not selected');
      return organizationsApi.deleteCommitIdentity(orgId);
    },
    onSuccess: invalidate,
    onError: callbacks?.onError,
  });

  return { saveCommitIdentity, removeCommitIdentity };
}
