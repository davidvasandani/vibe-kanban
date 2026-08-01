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
