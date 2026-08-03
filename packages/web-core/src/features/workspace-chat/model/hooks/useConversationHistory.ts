import {
  ExecutionProcess,
  ExecutionProcessStatus,
  PatchType,
} from 'shared/types';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { streamJsonPatchEntries } from '@/shared/lib/streamJsonPatchEntries';
import type {
  AddEntryType,
  ConversationTimelineSource,
  ExecutionProcessStateStore,
  PatchTypeWithKey,
  UseConversationHistoryParams,
} from '@/shared/hooks/useConversationHistory/types';

// Result type for the new UI's conversation history hook
export interface UseConversationHistoryResult {
  /** Whether the conversation only has a single coding agent turn (no follow-ups) */
  isFirstTurn: boolean;
  /** Whether completed processes exist before the currently loaded window. */
  hasEarlierHistory: boolean;
  /** Whether an explicitly requested older batch is loading. */
  isLoadingEarlier: boolean;
  /** Recoverable failure from the most recent older-history request. */
  loadEarlierError: string | null;
  /** Load one bounded batch of older completed processes. */
  loadEarlier: () => Promise<void>;
  /** Whether explicitly loaded older history can be released at the tail. */
  hasEvictableHistory: boolean;
  /** Release older batches while retaining the recent tail and live process. */
  releaseEarlierHistory: () => void;
}
import {
  MAX_RECENT_HISTORY_PROCESSES,
  MIN_INITIAL_ENTRIES,
  REMAINING_BATCH_SIZE,
} from '@/shared/hooks/useConversationHistory/constants';
import {
  getRecentProcessIdsToRetain,
  getUnloadedHistoricProcesses,
} from '../conversation-history-paging';

export const useConversationHistory = ({
  onTimelineUpdated,
  scopeKey,
}: UseConversationHistoryParams): UseConversationHistoryResult => {
  const {
    executionProcessesVisible: executionProcessesRaw,
    isLoading,
    isConnected,
  } = useExecutionProcessesContext();
  const executionProcesses = useRef<ExecutionProcess[]>(executionProcessesRaw);
  const displayedExecutionProcesses = useRef<ExecutionProcessStateStore>({});
  const loadedInitialEntries = useRef(false);
  const emittedEmptyInitialRef = useRef(false);
  const streamingProcessIdsRef = useRef<Set<string>>(new Set());
  const onTimelineUpdatedRef = useRef<
    UseConversationHistoryParams['onTimelineUpdated'] | null
  >(null);
  const previousStatusMapRef = useRef<Map<string, ExecutionProcessStatus>>(
    new Map()
  );
  const scopeGenerationRef = useRef(0);
  const loadEarlierInFlightRef = useRef(false);
  const [hasEarlierHistory, setHasEarlierHistory] = useState(false);
  const [isLoadingEarlier, setIsLoadingEarlier] = useState(false);
  const [loadEarlierError, setLoadEarlierError] = useState<string | null>(null);
  const [hasEvictableHistory, setHasEvictableHistory] = useState(false);

  // Derive whether this is the first turn (no follow-up processes exist)
  const isFirstTurn = useMemo(() => {
    const codingAgentProcessCount = executionProcessesRaw.filter(
      (ep) =>
        ep.executor_action.typ.type === 'CodingAgentInitialRequest' ||
        ep.executor_action.typ.type === 'CodingAgentFollowUpRequest'
    ).length;
    return codingAgentProcessCount <= 1;
  }, [executionProcessesRaw]);

  const mergeIntoDisplayed = (
    mutator: (state: ExecutionProcessStateStore) => void
  ) => {
    const state = displayedExecutionProcesses.current;
    mutator(state);
  };

  // The hook owns transport, loading, and reconciliation.
  // It emits a source model that later derivation layers can transform further.

  const buildTimelineSource = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore
    ): ConversationTimelineSource => ({
      executionProcessState,
      liveExecutionProcesses: executionProcesses.current,
    }),
    []
  );

  useEffect(() => {
    onTimelineUpdatedRef.current = onTimelineUpdated;
  }, [onTimelineUpdated]);

  // Keep executionProcesses up to date
  useEffect(() => {
    executionProcesses.current = executionProcessesRaw.filter(
      (ep) =>
        ep.run_reason === 'setupscript' ||
        ep.run_reason === 'cleanupscript' ||
        ep.run_reason === 'archivescript' ||
        ep.run_reason === 'codingagent'
    );
  }, [executionProcessesRaw]);

  const loadEntriesForHistoricExecutionProcess = (
    executionProcess: ExecutionProcess
  ) => {
    let url = '';
    if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
      url = `/api/execution-processes/${executionProcess.id}/raw-logs/ws`;
    } else {
      url = `/api/execution-processes/${executionProcess.id}/normalized-logs/ws`;
    }

    return new Promise<PatchType[]>((resolve, reject) => {
      const controller = streamJsonPatchEntries<PatchType>(url, {
        onFinished: (allEntries) => {
          controller.close();
          resolve(allEntries);
        },
        onError: (err) => {
          console.warn(
            `Error loading entries for historic execution process ${executionProcess.id}`,
            err
          );
          controller.close();
          reject(err);
        },
      });
    });
  };

  const patchWithKey = (
    patch: PatchType,
    executionProcessId: string,
    index: number
  ) => {
    return {
      ...patch,
      patchKey: `${executionProcessId}:${index}`,
      executionProcessId,
    };
  };

  const flattenEntries = (
    executionProcessState: ExecutionProcessStateStore
  ): PatchTypeWithKey[] => {
    return Object.values(executionProcessState)
      .filter(
        (p) =>
          p.executionProcess.executor_action.typ.type ===
            'CodingAgentFollowUpRequest' ||
          p.executionProcess.executor_action.typ.type ===
            'CodingAgentInitialRequest' ||
          p.executionProcess.executor_action.typ.type === 'ReviewRequest'
      )
      .sort(
        (a, b) =>
          new Date(
            a.executionProcess.created_at as unknown as string
          ).getTime() -
          new Date(b.executionProcess.created_at as unknown as string).getTime()
      )
      .flatMap((p) => p.entries);
  };

  const getActiveAgentProcesses = (): ExecutionProcess[] => {
    return (
      executionProcesses?.current.filter(
        (p) =>
          p.status === ExecutionProcessStatus.running &&
          p.run_reason !== 'devserver' &&
          p.run_reason !== 'backgroundhelper'
      ) ?? []
    );
  };

  const emitEntries = useCallback(
    (
      executionProcessState: ExecutionProcessStateStore,
      addEntryType: AddEntryType,
      loading: boolean
    ) => {
      const timelineSource = buildTimelineSource(executionProcessState);
      let modifiedAddEntryType = addEntryType;

      const latestEntry = Object.values(executionProcessState)
        .sort(
          (a, b) =>
            new Date(
              a.executionProcess.created_at as unknown as string
            ).getTime() -
            new Date(
              b.executionProcess.created_at as unknown as string
            ).getTime()
        )
        .flatMap((processState) => processState.entries)
        .at(-1);

      if (
        latestEntry?.type === 'NORMALIZED_ENTRY' &&
        latestEntry.content.entry_type.type === 'tool_use' &&
        latestEntry.content.entry_type.tool_name === 'ExitPlanMode'
      ) {
        modifiedAddEntryType = 'plan';
      }

      onTimelineUpdatedRef.current?.(
        timelineSource,
        modifiedAddEntryType,
        loading
      );
    },
    [buildTimelineSource]
  );

  // This emits its own events as they are streamed
  const loadRunningAndEmit = useCallback(
    (executionProcess: ExecutionProcess): Promise<void> => {
      return new Promise((resolve, reject) => {
        let url = '';
        if (executionProcess.executor_action.typ.type === 'ScriptRequest') {
          url = `/api/execution-processes/${executionProcess.id}/raw-logs/ws`;
        } else {
          url = `/api/execution-processes/${executionProcess.id}/normalized-logs/ws`;
        }
        const controller = streamJsonPatchEntries<PatchType>(url, {
          onEntries(entries) {
            const patchesWithKey = entries.map((entry, index) =>
              patchWithKey(entry, executionProcess.id, index)
            );
            mergeIntoDisplayed((state) => {
              state[executionProcess.id] = {
                executionProcess,
                entries: patchesWithKey,
              };
            });
            emitEntries(displayedExecutionProcesses.current, 'running', false);
          },
          onFinished: () => {
            emitEntries(displayedExecutionProcesses.current, 'running', false);
            controller.close();
            resolve();
          },
          onError: () => {
            controller.close();
            reject();
          },
        });
      });
    },
    [emitEntries]
  );

  // Sometimes it can take a few seconds for the stream to start, wrap the loadRunningAndEmit method
  const loadRunningAndEmitWithBackoff = useCallback(
    async (executionProcess: ExecutionProcess) => {
      for (let i = 0; i < 20; i++) {
        try {
          await loadRunningAndEmit(executionProcess);
          break;
        } catch (_) {
          await new Promise((resolve) => setTimeout(resolve, 500));
        }
      }
    },
    [loadRunningAndEmit]
  );

  const loadHistoricEntries = useCallback(
    async (maxEntries?: number): Promise<ExecutionProcessStateStore> => {
      const localDisplayedExecutionProcesses: ExecutionProcessStateStore = {};

      if (!executionProcesses?.current) return localDisplayedExecutionProcesses;

      for (const executionProcess of [
        ...executionProcesses.current,
      ].reverse()) {
        if (executionProcess.status === ExecutionProcessStatus.running)
          continue;

        let entries: PatchType[];
        try {
          entries =
            await loadEntriesForHistoricExecutionProcess(executionProcess);
        } catch {
          continue;
        }
        const entriesWithKey = entries.map((e, idx) =>
          patchWithKey(e, executionProcess.id, idx)
        );

        localDisplayedExecutionProcesses[executionProcess.id] = {
          executionProcess,
          entries: entriesWithKey,
        };

        if (
          maxEntries != null &&
          flattenEntries(localDisplayedExecutionProcesses).length > maxEntries
        ) {
          break;
        }
      }

      return localDisplayedExecutionProcesses;
    },
    [executionProcesses]
  );

  const hasUnloadedHistoricProcesses = useCallback((): boolean => {
    return (
      getUnloadedHistoricProcesses(
        executionProcesses.current,
        new Set(Object.keys(displayedExecutionProcesses.current))
      ).length > 0
    );
  }, []);

  const loadEarlierBatch = useCallback(
    async (
      batchSize: number
    ): Promise<{
      batch: ExecutionProcessStateStore;
      failedProcessCount: number;
    }> => {
      const batch: ExecutionProcessStateStore = {};
      let loadedEntryCount = 0;
      let failedProcessCount = 0;

      const unloadedProcesses = getUnloadedHistoricProcesses(
        executionProcesses.current,
        new Set(Object.keys(displayedExecutionProcesses.current))
      );
      for (const executionProcess of unloadedProcesses) {
        let entries: PatchType[];
        try {
          entries =
            await loadEntriesForHistoricExecutionProcess(executionProcess);
        } catch (error) {
          console.error(
            `Failed to load historic logs for process ${executionProcess.id}`,
            error
          );
          failedProcessCount += 1;
          continue;
        }
        const entriesWithKey = entries.map((e, idx) =>
          patchWithKey(e, executionProcess.id, idx)
        );

        batch[executionProcess.id] = {
          executionProcess,
          entries: entriesWithKey,
        };

        loadedEntryCount += entriesWithKey.length;
        if (loadedEntryCount >= batchSize) break;
      }

      return { batch, failedProcessCount };
    },
    []
  );

  const loadEarlier = useCallback(async (): Promise<void> => {
    if (loadEarlierInFlightRef.current || !hasUnloadedHistoricProcesses()) {
      return;
    }

    const generation = scopeGenerationRef.current;
    loadEarlierInFlightRef.current = true;
    setIsLoadingEarlier(true);
    setLoadEarlierError(null);

    try {
      const { batch, failedProcessCount } =
        await loadEarlierBatch(REMAINING_BATCH_SIZE);
      if (generation !== scopeGenerationRef.current) return;

      mergeIntoDisplayed((state) => {
        Object.assign(state, batch);
      });
      if (Object.keys(batch).length > 0) setHasEvictableHistory(true);
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
      setHasEarlierHistory(hasUnloadedHistoricProcesses());
      if (failedProcessCount > 0) {
        setLoadEarlierError('Some earlier messages could not be loaded');
      }
    } catch (error) {
      if (generation !== scopeGenerationRef.current) return;
      setLoadEarlierError(
        error instanceof Error
          ? error.message
          : 'Unable to load earlier messages'
      );
    } finally {
      if (generation === scopeGenerationRef.current) {
        loadEarlierInFlightRef.current = false;
        setIsLoadingEarlier(false);
      }
    }
  }, [emitEntries, hasUnloadedHistoricProcesses, loadEarlierBatch]);

  const releaseEarlierHistory = useCallback(() => {
    if (!hasEvictableHistory || loadEarlierInFlightRef.current) return;

    const loadedConversationEntryCounts = new Map(
      Object.values(displayedExecutionProcesses.current).map((processState) => [
        processState.executionProcess.id,
        flattenEntries({
          [processState.executionProcess.id]: processState,
        }).length,
      ])
    );
    const retainedIds = getRecentProcessIdsToRetain(
      executionProcesses.current,
      loadedConversationEntryCounts,
      MIN_INITIAL_ENTRIES,
      MAX_RECENT_HISTORY_PROCESSES
    );
    let removedAny = false;

    mergeIntoDisplayed((state) => {
      for (const processId of Object.keys(state)) {
        if (retainedIds.has(processId)) continue;
        delete state[processId];
        removedAny = true;
      }
    });

    setHasEvictableHistory(false);
    setHasEarlierHistory(hasUnloadedHistoricProcesses());
    if (removedAny) {
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
    }
  }, [emitEntries, hasEvictableHistory, hasUnloadedHistoricProcesses]);

  const ensureProcessVisible = useCallback((p: ExecutionProcess) => {
    mergeIntoDisplayed((state) => {
      if (!state[p.id]) {
        state[p.id] = {
          executionProcess: {
            id: p.id,
            created_at: p.created_at,
            updated_at: p.updated_at,
            executor_action: p.executor_action,
          },
          entries: [],
        };
      }
    });
  }, []);

  const idListKey = useMemo(
    () => executionProcessesRaw?.map((p) => p.id).join(','),
    [executionProcessesRaw]
  );

  const idStatusKey = useMemo(
    () => executionProcessesRaw?.map((p) => `${p.id}:${p.status}`).join(','),
    [executionProcessesRaw]
  );

  // Clean up entries for processes that have been removed (e.g., after reset)
  useEffect(() => {
    if (isLoading || !isConnected) return;
    const visibleProcessIds = new Set(executionProcessesRaw.map((p) => p.id));
    const displayedIds = Object.keys(displayedExecutionProcesses.current);
    let changed = false;

    for (const id of displayedIds) {
      if (!visibleProcessIds.has(id)) {
        delete displayedExecutionProcesses.current[id];
        changed = true;
      }
    }

    if (changed) {
      emitEntries(displayedExecutionProcesses.current, 'historic', false);
    }
  }, [idListKey, executionProcessesRaw, emitEntries, isLoading, isConnected]);

  useEffect(() => {
    scopeGenerationRef.current += 1;
    displayedExecutionProcesses.current = {};
    loadedInitialEntries.current = false;
    emittedEmptyInitialRef.current = false;
    streamingProcessIdsRef.current.clear();
    previousStatusMapRef.current.clear();
    loadEarlierInFlightRef.current = false;
    setHasEarlierHistory(false);
    setIsLoadingEarlier(false);
    setLoadEarlierError(null);
    setHasEvictableHistory(false);
    emitEntries(displayedExecutionProcesses.current, 'initial', true);
  }, [scopeKey, emitEntries]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (loadedInitialEntries.current) return;

      if (isLoading) return;

      if (executionProcesses.current.length === 0) {
        if (emittedEmptyInitialRef.current) return;
        emittedEmptyInitialRef.current = true;
        emitEntries(displayedExecutionProcesses.current, 'initial', false);
        return;
      }

      emittedEmptyInitialRef.current = false;

      const allInitialEntries = await loadHistoricEntries(MIN_INITIAL_ENTRIES);
      if (cancelled) return;
      loadedInitialEntries.current = true;
      mergeIntoDisplayed((state) => {
        Object.assign(state, allInitialEntries);
      });
      emitEntries(displayedExecutionProcesses.current, 'initial', false);
      setHasEarlierHistory(hasUnloadedHistoricProcesses());
    })();
    return () => {
      cancelled = true;
    };
  }, [
    scopeKey,
    idListKey,
    isLoading,
    loadHistoricEntries,
    hasUnloadedHistoricProcesses,
    emitEntries,
  ]); // include idListKey so new processes trigger reload

  useEffect(() => {
    const activeProcesses = getActiveAgentProcesses();
    if (activeProcesses.length === 0) return;

    for (const activeProcess of activeProcesses) {
      if (!displayedExecutionProcesses.current[activeProcess.id]) {
        const runningOrInitial =
          Object.keys(displayedExecutionProcesses.current).length > 1
            ? 'running'
            : 'initial';
        ensureProcessVisible(activeProcess);
        emitEntries(
          displayedExecutionProcesses.current,
          runningOrInitial,
          false
        );
      }

      if (
        activeProcess.status === ExecutionProcessStatus.running &&
        !streamingProcessIdsRef.current.has(activeProcess.id)
      ) {
        streamingProcessIdsRef.current.add(activeProcess.id);
        loadRunningAndEmitWithBackoff(activeProcess).finally(() => {
          streamingProcessIdsRef.current.delete(activeProcess.id);
        });
      }
    }
  }, [
    scopeKey,
    idStatusKey,
    emitEntries,
    ensureProcessVisible,
    loadRunningAndEmitWithBackoff,
  ]);

  useEffect(() => {
    if (!executionProcessesRaw) return;

    const processesToReload: ExecutionProcess[] = [];

    for (const process of executionProcessesRaw) {
      const previousStatus = previousStatusMapRef.current.get(process.id);
      const currentStatus = process.status;

      if (
        previousStatus === ExecutionProcessStatus.running &&
        currentStatus !== ExecutionProcessStatus.running &&
        displayedExecutionProcesses.current[process.id]
      ) {
        processesToReload.push(process);
      }

      previousStatusMapRef.current.set(process.id, currentStatus);
    }

    if (processesToReload.length === 0) return;

    (async () => {
      let anyUpdated = false;

      for (const process of processesToReload) {
        let entries: PatchType[];
        try {
          entries = await loadEntriesForHistoricExecutionProcess(process);
        } catch {
          continue;
        }
        if (entries.length === 0) continue;

        const entriesWithKey = entries.map((e, idx) =>
          patchWithKey(e, process.id, idx)
        );

        mergeIntoDisplayed((state) => {
          state[process.id] = {
            executionProcess: process,
            entries: entriesWithKey,
          };
        });
        anyUpdated = true;
      }

      if (anyUpdated) {
        emitEntries(displayedExecutionProcesses.current, 'running', false);
      }
    })();
  }, [idStatusKey, executionProcessesRaw, emitEntries]);

  // If an execution process is removed, remove it from the state
  useEffect(() => {
    if (!executionProcessesRaw) return;

    const removedProcessIds = Object.keys(
      displayedExecutionProcesses.current
    ).filter((id) => !executionProcessesRaw.some((p) => p.id === id));

    if (removedProcessIds.length > 0) {
      mergeIntoDisplayed((state) => {
        removedProcessIds.forEach((id) => {
          delete state[id];
        });
      });
    }
  }, [scopeKey, idListKey, executionProcessesRaw]);

  return {
    isFirstTurn,
    hasEarlierHistory,
    isLoadingEarlier,
    loadEarlierError,
    loadEarlier,
    hasEvictableHistory,
    releaseEarlierHistory,
  };
};
