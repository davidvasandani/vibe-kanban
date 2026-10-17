import { useCallback } from 'react';
import { useJsonPatchWsStream } from '@/shared/hooks/useJsonPatchWsStream';
import { useHostId } from '@/shared/providers/HostIdProvider';
import type { ExecutionProcess } from 'shared/types';

type ExecutionProcessState = {
  execution_processes: Record<string, ExecutionProcess>;
};

interface UseExecutionProcessesResult {
  executionProcesses: ExecutionProcess[];
  executionProcessesById: Record<string, ExecutionProcess>;
  isAttemptRunning: boolean;
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

export function hasRunningAttempt(
  executionProcesses: Pick<ExecutionProcess, 'run_reason' | 'status'>[]
): boolean {
  return executionProcesses.some(
    (process) =>
      (process.run_reason === 'codingagent' ||
        process.run_reason === 'setupscript' ||
        process.run_reason === 'cleanupscript' ||
        process.run_reason === 'archivescript') &&
      process.status === 'running'
  );
}

/**
 * The session process stream URL. Always the soft-deleted superset: the
 * server's `show_soft_deleted=false` is exactly a `!dropped` filter, so every
 * caller can share this one socket and filter locally (constitution XLIV).
 */
export function sessionExecutionProcessesEndpoint(
  sessionId: string,
  hostId: string | null
): string {
  const apiBasePath = hostId ? `/api/host/${hostId}` : '/api';
  const params = new URLSearchParams({
    session_id: sessionId,
    show_soft_deleted: 'true',
  });
  return `${apiBasePath}/execution-processes/stream/session/ws?${params.toString()}`;
}

/**
 * Stream execution processes for a session via WebSocket (JSON Patch) and expose as array + map.
 * Server sends initial snapshot: replace /execution_processes with an object keyed by id.
 * Live updates arrive at /execution_processes/<id> via add/replace/remove operations.
 */
export const useExecutionProcesses = (
  sessionId: string | undefined,
  opts?: { showSoftDeleted?: boolean }
): UseExecutionProcessesResult => {
  const hostId = useHostId();
  const showSoftDeleted = opts?.showSoftDeleted === true;
  const endpoint = sessionId
    ? sessionExecutionProcessesEndpoint(sessionId, hostId)
    : undefined;

  const initialData = useCallback(
    (): ExecutionProcessState => ({ execution_processes: {} }),
    []
  );

  const { data, isConnected, isInitialized, error } =
    useJsonPatchWsStream<ExecutionProcessState>(
      endpoint,
      !!sessionId,
      initialData
    );

  const streamedExecutionProcesses = Object.values(
    data?.execution_processes ?? {}
  ).sort(
    (a, b) =>
      new Date(a.created_at as unknown as string).getTime() -
      new Date(b.created_at as unknown as string).getTime()
  );

  // Guard against stale buffered stream data when switching sessions quickly,
  // and apply the soft-delete filter the server would have applied.
  const executionProcesses = streamedExecutionProcesses.filter(
    (executionProcess) =>
      (!sessionId || executionProcess.session_id === sessionId) &&
      (showSoftDeleted || !executionProcess.dropped)
  );

  const executionProcessesById = executionProcesses.reduce<
    Record<string, ExecutionProcess>
  >((processesById, executionProcess) => {
    processesById[executionProcess.id] = executionProcess;
    return processesById;
  }, {});

  const isAttemptRunning = hasRunningAttempt(executionProcesses);
  const isLoading = !!sessionId && !isInitialized && !error; // until first snapshot

  return {
    executionProcesses,
    executionProcessesById,
    isAttemptRunning,
    isLoading,
    isConnected,
    error,
  };
};
