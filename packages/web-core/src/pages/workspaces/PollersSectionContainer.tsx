import { useMemo, useState } from 'react';
import { toast } from 'sonner';
import { StopIcon } from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import { ExecutionProcessStatus } from 'shared/types';
import type { ExecutionProcess } from 'shared/types';
import { executionProcessesApi } from '@/shared/lib/api';
import {
  selectPollers,
  type WorkspacePoller,
} from '@/pages/workspaces/pollers';

/**
 * Each status renders as itself. A killed poller (someone stopped it) and a
 * failed poller (it died) are different facts, and collapsing them into a
 * generic "stopped" would hide the one worth acting on.
 */
const STATUS_LABEL: Record<ExecutionProcess['status'], string> = {
  running: 'Running',
  completed: 'Finished',
  failed: 'Failed',
  killed: 'Stopped',
  interrupted: 'Interrupted',
  indeterminate: 'Unknown',
};

const STATUS_CLASS: Record<ExecutionProcess['status'], string> = {
  running: 'text-low',
  completed: 'text-low',
  failed: 'text-destructive',
  killed: 'text-low',
  interrupted: 'text-low',
  indeterminate: 'text-low',
};

function formatInterval(intervalSecs: number): string {
  if (intervalSecs % 3600 === 0) return `every ${intervalSecs / 3600}h`;
  if (intervalSecs % 60 === 0) return `every ${intervalSecs / 60}m`;
  return `every ${intervalSecs}s`;
}

function formatStartedAt(startedAt: string): string {
  const started = new Date(startedAt);
  if (Number.isNaN(started.getTime())) return '';
  return started.toLocaleString();
}

function PollerRow({ poller }: { poller: WorkspacePoller }) {
  const [isStopping, setIsStopping] = useState(false);
  const canStop = poller.status === ExecutionProcessStatus.running;

  const stop = async () => {
    setIsStopping(true);
    try {
      await executionProcessesApi.stopExecutionProcess(poller.id);
    } catch (error) {
      // Surface the reason rather than a generic failure: the operator needs to
      // know whether the poller is still running.
      toast.error(
        `Failed to stop poller: ${
          error instanceof Error ? error.message : String(error)
        }`
      );
    } finally {
      setIsStopping(false);
    }
  };

  return (
    <li className="flex items-start gap-2 px-3 py-2">
      <div className="min-w-0 flex-1">
        <code
          className="block truncate text-xs"
          title={poller.command}
          data-testid="poller-command"
        >
          {poller.command}
        </code>
        <div className="mt-0.5 flex flex-wrap gap-x-2 text-xs text-low">
          <span>{formatInterval(poller.intervalSecs)}</span>
          <span
            className={STATUS_CLASS[poller.status]}
            data-testid="poller-status"
          >
            {STATUS_LABEL[poller.status]}
          </span>
          <span title={poller.startedAt}>
            started {formatStartedAt(poller.startedAt)}
          </span>
        </div>
      </div>
      {canStop && (
        <Button
          variant="ghost"
          size="sm"
          onClick={stop}
          disabled={isStopping}
          aria-label={`Stop poller: ${poller.command}`}
        >
          <StopIcon className="h-4 w-4" />
        </Button>
      )}
    </li>
  );
}

interface PollersSectionContainerProps {
  executionProcesses: ExecutionProcess[];
}

/**
 * Lists the workspace's vk pollers.
 *
 * Derived entirely from the execution processes already streamed for this
 * session — the section makes no request of its own, expanded or collapsed.
 */
export function PollersSectionContainer({
  executionProcesses,
}: PollersSectionContainerProps) {
  const pollers = useMemo(
    () => selectPollers(executionProcesses),
    [executionProcesses]
  );

  if (pollers.length === 0) {
    return (
      <p className="px-3 py-2 text-xs text-low">
        No pollers. An agent can start one with the <code>spawn_poller</code>{' '}
        tool; it keeps running after the agent&apos;s turn ends.
      </p>
    );
  }

  return (
    <ul className="divide-y" data-testid="pollers-list">
      {pollers.map((poller) => (
        <PollerRow key={poller.id} poller={poller} />
      ))}
    </ul>
  );
}
