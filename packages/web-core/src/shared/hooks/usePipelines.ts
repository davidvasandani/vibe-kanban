import { useQuery } from '@tanstack/react-query';
import type { Pipeline } from 'shared/types';
import { pipelinesApi } from '@/shared/lib/api';

export const PIPELINES_QUERY_KEY = ['pipelines'] as const;

/**
 * Fetch the file-based task pipelines exposed by GET /api/pipelines.
 * Pipelines live in local TOML files and change rarely, so a short
 * stale time keeps the task-create flow snappy without going stale.
 */
export function usePipelines() {
  return useQuery<Pipeline[]>({
    queryKey: PIPELINES_QUERY_KEY,
    queryFn: () => pipelinesApi.list(),
    staleTime: 60 * 1000,
  });
}
