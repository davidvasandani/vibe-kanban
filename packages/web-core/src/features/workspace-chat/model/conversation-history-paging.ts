import { ExecutionProcess, ExecutionProcessStatus } from 'shared/types';

export interface LoadedProcessEntries<TEntry> {
  process: ExecutionProcess;
  entries: TEntry[];
}

export interface LoadProcessesResult<TEntry> {
  loaded: LoadedProcessEntries<TEntry>[];
  failedProcessCount: number;
}

/**
 * Fetch `processes` in order, `concurrency` at a time, stopping at the first
 * process after which `isEnough` holds.
 *
 * Fetching a completed process is expensive on the server: its normalized log
 * is not stored, so each request reloads the whole raw log and reruns the
 * vendor normalizer. Doing that one process at a time made opening a long
 * conversation take minutes, since the wait is the *sum* of every process in
 * the window rather than the slowest one. Nothing about the window depends on
 * an earlier response, so the requests overlap.
 *
 * Two properties are load-bearing and pinned by tests:
 *
 * - **Results commit in request order**, never completion order. The window is
 *   "the newest processes until the threshold is crossed", so which response
 *   happens to arrive first must not change where it stops or how the
 *   conversation is ordered.
 * - **The stopping point is unchanged from fetching serially.** Up to
 *   `concurrency - 1` extra responses may be in flight when the threshold is
 *   crossed; those are discarded rather than shown, so concurrency tunes
 *   latency and never what the reader sees.
 *
 * A process that fails is skipped and counted, matching the previous
 * behaviour: one unreadable turn must not cost the reader the rest of them.
 */
export async function loadProcessesInOrder<TEntry>(
  processes: ExecutionProcess[],
  fetchEntries: (process: ExecutionProcess) => Promise<TEntry[]>,
  isEnough: (loaded: LoadedProcessEntries<TEntry>[]) => boolean,
  concurrency: number
): Promise<LoadProcessesResult<TEntry>> {
  const loaded: LoadedProcessEntries<TEntry>[] = [];
  let failedProcessCount = 0;

  for (let start = 0; start < processes.length; start += concurrency) {
    const slice = processes.slice(start, start + concurrency);
    const settled = await Promise.all(
      slice.map((process) =>
        fetchEntries(process).then(
          (entries) => ({ ok: true as const, process, entries }),
          () => ({ ok: false as const, process })
        )
      )
    );

    for (const result of settled) {
      if (!result.ok) {
        failedProcessCount += 1;
        continue;
      }
      loaded.push({ process: result.process, entries: result.entries });
      if (isEnough(loaded)) {
        return { loaded, failedProcessCount };
      }
    }
  }

  return { loaded, failedProcessCount };
}

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
