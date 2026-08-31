import { ExecutionProcessStatus } from 'shared/types';
import type { ExecutionProcess } from 'shared/types';

/**
 * A vk poller, projected out of the execution-process stream.
 *
 * A poller is a background helper whose generated script Vibe Kanban compiled
 * from a `PollerSpec`, so it is identified by run reason *and* the presence of
 * that spec — a plain background helper carries `poller: null` and is excluded.
 */
export interface WorkspacePoller {
  id: string;
  command: string;
  intervalSecs: number;
  status: ExecutionProcess['status'];
  startedAt: string;
}

/**
 * Project the pollers out of the execution processes already being streamed for
 * the workspace's session.
 *
 * Deliberately derived rather than fetched: the drawer must not issue a request
 * of its own to populate or label itself, and every field it needs is already on
 * the streamed row.
 */
export function selectPollers(
  executionProcesses: ExecutionProcess[]
): WorkspacePoller[] {
  return executionProcesses.flatMap((process) => {
    if (process.run_reason !== 'backgroundhelper') return [];

    const action = process.executor_action?.typ;
    if (action?.type !== 'ScriptRequest' || !action.poller) return [];

    return [
      {
        id: process.id,
        command: action.poller.command,
        intervalSecs: action.poller.interval_secs,
        status: process.status,
        startedAt: process.started_at,
      },
    ];
  });
}

export interface PollersHeaderStatus {
  visibleText: string;
  accessibleText: string;
  hasFailure: boolean;
}

const RUNNING = ExecutionProcessStatus.running;
const FAILED = ExecutionProcessStatus.failed;

/**
 * The collapsed-header summary.
 *
 * Reports the running count, and surfaces failure *distinctly* — a bare running
 * count would render a workspace whose only poller just died identically to one
 * that never had a poller at all, which is the state a user most needs to notice
 * before deciding whether to open the section.
 *
 * Returns `null` when there is nothing decisive to say, so the header shows
 * nothing rather than a zero.
 */
export function derivePollersHeaderStatus(
  pollers: WorkspacePoller[]
): PollersHeaderStatus | null {
  const running = pollers.filter((poller) => poller.status === RUNNING).length;
  const failed = pollers.filter((poller) => poller.status === FAILED).length;

  if (running === 0 && failed === 0) return null;

  const parts: string[] = [];
  const accessibleParts: string[] = [];

  if (running > 0) {
    parts.push(String(running));
    accessibleParts.push(
      `${running} ${running === 1 ? 'poller' : 'pollers'} running`
    );
  }
  if (failed > 0) {
    parts.push(`${failed} failed`);
    accessibleParts.push(
      `${failed} ${failed === 1 ? 'poller' : 'pollers'} failed`
    );
  }

  return {
    visibleText: parts.join(' · '),
    accessibleText: accessibleParts.join('; '),
    hasFailure: failed > 0,
  };
}
