import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  hasRunningAttempt,
  useExecutionProcesses,
} from '@/shared/hooks/useExecutionProcesses';
import type { ExecutionProcess } from 'shared/types';
import {
  ExecutionProcessesContext,
  type ExecutionProcessesContextType,
} from '@/shared/hooks/useExecutionProcessesContext';

export const ExecutionProcessesProvider: React.FC<{
  sessionId?: string | undefined;
  children: React.ReactNode;
}> = ({ sessionId, children }) => {
  const { executionProcesses, isLoading, isConnected, error } =
    useExecutionProcesses(sessionId, { showSoftDeleted: true });

  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;
  const [responseProcessesById, setResponseProcessesById] = useState<
    Record<string, ExecutionProcess>
  >({});

  const reconcileExecutionProcess = useCallback((process: ExecutionProcess) => {
    if (!sessionIdRef.current || process.session_id !== sessionIdRef.current) {
      return;
    }
    setResponseProcessesById((current) => {
      if (current[process.id] === process) return current;
      return { ...current, [process.id]: process };
    });
  }, []);

  useEffect(() => {
    setResponseProcessesById({});
  }, [sessionId]);

  useEffect(() => {
    const streamedIds = new Set(
      executionProcesses.map((process) => process.id)
    );
    if (streamedIds.size === 0) return;

    setResponseProcessesById((current) => {
      const remaining = Object.fromEntries(
        Object.entries(current).filter(([id]) => !streamedIds.has(id))
      );
      return Object.keys(remaining).length === Object.keys(current).length
        ? current
        : remaining;
    });
  }, [executionProcesses]);

  const reconciledExecutionProcesses = useMemo(() => {
    const byId: Record<string, ExecutionProcess> = {};
    for (const process of Object.values(responseProcessesById)) {
      if (process.session_id === sessionId) byId[process.id] = process;
    }
    for (const process of executionProcesses) byId[process.id] = process;
    return Object.values(byId).sort(
      (a, b) =>
        new Date(a.created_at as unknown as string).getTime() -
        new Date(b.created_at as unknown as string).getTime()
    );
  }, [executionProcesses, responseProcessesById, sessionId]);

  const reconciledExecutionProcessesById = useMemo(
    () =>
      Object.fromEntries(
        reconciledExecutionProcesses.map((process) => [process.id, process])
      ),
    [reconciledExecutionProcesses]
  );

  const visible = useMemo(() => {
    return reconciledExecutionProcesses.filter((p) => !p.dropped);
  }, [reconciledExecutionProcesses]);

  const executionProcessesByIdVisible = useMemo(() => {
    const m: Record<string, ExecutionProcess> = {};
    for (const p of visible) m[p.id] = p;
    return m;
  }, [visible]);

  const isAttemptRunningVisible = useMemo(
    () => hasRunningAttempt(visible),
    [visible]
  );

  const value = useMemo<ExecutionProcessesContextType>(
    () => ({
      executionProcessesAll: reconciledExecutionProcesses,
      executionProcessesByIdAll: reconciledExecutionProcessesById,
      isAttemptRunningAll: hasRunningAttempt(reconciledExecutionProcesses),
      executionProcessesVisible: visible,
      executionProcessesByIdVisible,
      isAttemptRunningVisible,
      isLoading,
      isConnected,
      error,
      reconcileExecutionProcess,
    }),
    [
      reconciledExecutionProcesses,
      reconciledExecutionProcessesById,
      visible,
      executionProcessesByIdVisible,
      isAttemptRunningVisible,
      isLoading,
      isConnected,
      error,
      reconcileExecutionProcess,
    ]
  );

  return (
    <ExecutionProcessesContext.Provider value={value}>
      {children}
    </ExecutionProcessesContext.Provider>
  );
};
