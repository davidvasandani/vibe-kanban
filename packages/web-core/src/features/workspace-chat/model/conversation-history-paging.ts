import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';

/**
 * Return unloaded completed processes newest-first, matching the order in
 * which conversation history should be fetched while the user scrolls up.
 * Running processes have an independent live stream and never belong to an
 * older-history batch.
 */
export function getUnloadedHistoricProcesses(
  processes: ExecutionProcess[],
  loadedProcessIds: ReadonlySet<string>
): ExecutionProcess[] {
  return processes
    .filter(
      (process) =>
        process.status !== ExecutionProcessStatus.running &&
        !loadedProcessIds.has(process.id)
    )
    .reverse();
}

/**
 * Select the loaded process window that recreates the recent-tail view.
 * Running processes are always retained. Historic processes are retained
 * newest-first until the same visible-entry threshold used by initial loading
 * has been crossed.
 */
export function getRecentProcessIdsToRetain(
  processes: ExecutionProcess[],
  loadedConversationEntryCounts: ReadonlyMap<string, number>,
  minimumEntryCount: number,
  maximumHistoricProcessCount: number
): Set<string> {
  const retainedIds = new Set<string>();
  let retainedEntryCount = 0;
  let retainedHistoricProcessCount = 0;

  for (const process of [...processes].reverse()) {
    if (!loadedConversationEntryCounts.has(process.id)) continue;

    if (process.status === ExecutionProcessStatus.running) {
      retainedIds.add(process.id);
      continue;
    }

    if (
      retainedEntryCount > minimumEntryCount ||
      retainedHistoricProcessCount >= maximumHistoricProcessCount
    ) {
      continue;
    }

    retainedIds.add(process.id);
    retainedHistoricProcessCount += 1;
    retainedEntryCount += loadedConversationEntryCounts.get(process.id) ?? 0;
  }

  return retainedIds;
}
