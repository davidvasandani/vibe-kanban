import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import type { MachineClient } from '@/shared/lib/machineClient';
import {
  PIPELINES_QUERY_KEY,
  getPipelineRawQueryKey,
  getPipelineStatusQueryKey,
  invalidatePipelineSettingsQueries,
} from './usePipelines';

function machine(id: string): MachineClient {
  return {
    target: {
      kind: id === 'local' ? 'local' : 'remote',
      id,
      apiHostId: id === 'local' ? null : id,
      label: id,
    },
    queryScopeKey: ['machine', id] as const,
  } as MachineClient;
}

describe('pipeline settings query keys', () => {
  it('keys status and raw queries by machine scope', () => {
    const hostA = machine('host-a');
    const hostB = machine('host-b');

    expect(getPipelineStatusQueryKey(hostA)).toEqual([
      'pipeline-statuses',
      'machine',
      'host-a',
    ]);
    expect(getPipelineStatusQueryKey(hostB)).toEqual([
      'pipeline-statuses',
      'machine',
      'host-b',
    ]);
    expect(getPipelineRawQueryKey(hostA, 'basic')).toEqual([
      'pipeline-raw',
      'machine',
      'host-a',
      'basic',
    ]);
  });

  it('uses the unselected machine scope for disabled queries', () => {
    expect(getPipelineStatusQueryKey(null)).toEqual([
      'pipeline-statuses',
      'machine',
      'unselected',
    ]);
    expect(getPipelineRawQueryKey(null, null)).toEqual([
      'pipeline-raw',
      'machine',
      'unselected',
      null,
    ]);
  });

  it('invalidates same-scope status/raw keys and the legacy catalog', () => {
    const queryClient = new QueryClient();
    const invalidate = vi
      .spyOn(queryClient, 'invalidateQueries')
      .mockResolvedValue(undefined);

    invalidatePipelineSettingsQueries(queryClient, machine('host-a'));

    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ['pipeline-statuses', 'machine', 'host-a'],
    });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ['pipeline-raw', 'machine', 'host-a'],
    });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: PIPELINES_QUERY_KEY,
    });
  });
});
