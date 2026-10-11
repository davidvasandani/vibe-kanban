import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query';
import type {
  NewIssuePipelineDefaultsResponse,
  Pipeline,
  PipelineFileStatus,
  PipelineRawBody,
  PipelineValidateBody,
  PipelineValidation,
} from 'shared/types';
import { pipelinesApi } from '@/shared/lib/api';
import { getHostRequestScopeQueryKey } from '@/shared/lib/hostRequestScope';
import { useHostId } from '@/shared/providers/HostIdProvider';
import type { MachineClient } from '@/shared/lib/machineClient';

export const PIPELINES_QUERY_KEY = ['pipelines'] as const;
export const DISABLED_MACHINE_SCOPE = ['machine', 'unselected'] as const;

export const pipelineSettingsKeys = {
  statuses: (scope: readonly ['machine', string]) =>
    ['pipeline-statuses', ...scope] as const,
  raw: (scope: readonly ['machine', string], pipelineId: string | null) =>
    ['pipeline-raw', ...scope, pipelineId] as const,
};

/**
 * Fetch the file-based task pipelines exposed by GET /api/pipelines.
 * Pipelines live in local TOML files and change rarely, so a short
 * stale time keeps the task-create flow snappy without going stale.
 */
export function usePipelines() {
  // Pipelines are per-machine files, so the cache is per host.
  const hostId = useHostId();
  return useQuery<Pipeline[]>({
    queryKey: [...PIPELINES_QUERY_KEY, getHostRequestScopeQueryKey(hostId)],
    queryFn: () => pipelinesApi.list(hostId),
    staleTime: 60 * 1000,
  });
}

/**
 * Resolve the pipelines a new issue in `projectId` starts with (the
 * project's override, else its organization's, else the machine default;
 * see `GET /api/pipelines/new-issue-defaults`). Keyed under
 * `PIPELINES_QUERY_KEY`, so pipeline-settings invalidation refreshes it.
 */
export function useNewIssuePipelineDefaults(
  projectId: string | undefined,
  enabled = true
) {
  // Defaults live in the host's config, so scope the cache (and target the
  // request) to the host the issue is being created on.
  const hostId = useHostId();
  return useQuery<NewIssuePipelineDefaultsResponse>({
    queryKey: [
      ...PIPELINES_QUERY_KEY,
      'new-issue-defaults',
      getHostRequestScopeQueryKey(hostId),
      projectId ?? null,
    ],
    queryFn: () => pipelinesApi.newIssueDefaults(projectId, hostId),
    enabled,
    staleTime: 60 * 1000,
    retry: 1,
  });
}

function scopeForClient(machineClient: MachineClient | null) {
  return machineClient?.queryScopeKey ?? DISABLED_MACHINE_SCOPE;
}

export function getPipelineStatusQueryKey(machineClient: MachineClient | null) {
  return pipelineSettingsKeys.statuses(scopeForClient(machineClient));
}

export function getPipelineRawQueryKey(
  machineClient: MachineClient | null,
  pipelineId: string | null
) {
  return pipelineSettingsKeys.raw(scopeForClient(machineClient), pipelineId);
}

export function usePipelineStatuses(machineClient: MachineClient | null) {
  return useQuery<PipelineFileStatus[]>({
    queryKey: getPipelineStatusQueryKey(machineClient),
    queryFn: () => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.listPipelineStatuses();
    },
    enabled: Boolean(machineClient),
    staleTime: 10 * 1000,
  });
}

export function usePipelineRaw(
  machineClient: MachineClient | null,
  pipelineId: string | null
) {
  return useQuery<string>({
    queryKey: getPipelineRawQueryKey(machineClient, pipelineId),
    queryFn: () => {
      if (!machineClient || !pipelineId) {
        throw new Error('No pipeline selected');
      }
      return machineClient.readPipelineRaw(pipelineId);
    },
    enabled: Boolean(machineClient && pipelineId),
    staleTime: 0,
  });
}

export function invalidatePipelineSettingsQueries(
  queryClient: QueryClient,
  machineClient: MachineClient
) {
  const scope = machineClient.queryScopeKey;
  void queryClient.invalidateQueries({
    queryKey: pipelineSettingsKeys.statuses(scope),
  });
  void queryClient.invalidateQueries({
    queryKey: ['pipeline-raw', ...scope],
  });
  void queryClient.invalidateQueries({ queryKey: PIPELINES_QUERY_KEY });
}

export function useValidatePipelineMutation(
  machineClient: MachineClient | null
) {
  return useMutation<PipelineValidation, Error, PipelineValidateBody>({
    mutationFn: (body) => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.validatePipeline(body);
    },
  });
}

export function useWritePipelineRawMutation(
  machineClient: MachineClient | null
) {
  const queryClient = useQueryClient();
  return useMutation<Pipeline, Error, { id: string; body: PipelineRawBody }>({
    mutationFn: ({ id, body }) => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.writePipelineRaw(id, body);
    },
    onSuccess: () => {
      if (machineClient) {
        invalidatePipelineSettingsQueries(queryClient, machineClient);
      }
    },
  });
}

export function useResetPipelineMutation(machineClient: MachineClient | null) {
  const queryClient = useQueryClient();
  return useMutation<Pipeline, Error, string>({
    mutationFn: (id) => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.resetPipeline(id);
    },
    onSuccess: () => {
      if (machineClient) {
        invalidatePipelineSettingsQueries(queryClient, machineClient);
      }
    },
  });
}

export function useResetDefaultPipelinesMutation(
  machineClient: MachineClient | null
) {
  const queryClient = useQueryClient();
  return useMutation<Pipeline[], Error, void>({
    mutationFn: () => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.resetDefaultPipelines();
    },
    onSuccess: () => {
      if (machineClient) {
        invalidatePipelineSettingsQueries(queryClient, machineClient);
      }
    },
  });
}

export function useDeletePipelineMutation(machineClient: MachineClient | null) {
  const queryClient = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (id) => {
      if (!machineClient) {
        throw new Error('No machine selected');
      }
      return machineClient.deletePipeline(id);
    },
    onSuccess: () => {
      if (machineClient) {
        invalidatePipelineSettingsQueries(queryClient, machineClient);
      }
    },
  });
}
