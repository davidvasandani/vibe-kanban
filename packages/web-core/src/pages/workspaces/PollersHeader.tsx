import { useMemo } from 'react';
import type { ExecutionProcess } from 'shared/types';
import {
  derivePollersHeaderStatus,
  selectPollers,
} from '@/pages/workspaces/pollers';

interface PollersHeaderProps {
  executionProcesses: ExecutionProcess[];
}

/**
 * Collapsed-state summary for the Pollers section.
 *
 * Renders inside the section header row, which survives collapse, so the count
 * stays current while the body is unmounted. It takes the already-streamed
 * execution processes as a prop and issues no request of its own.
 */
export function PollersHeader({ executionProcesses }: PollersHeaderProps) {
  const status = useMemo(
    () => derivePollersHeaderStatus(selectPollers(executionProcesses)),
    [executionProcesses]
  );

  if (!status) return null;

  return (
    <span
      className={`min-w-0 max-w-28 truncate text-sm ${
        status.hasFailure ? 'text-destructive' : 'text-low'
      }`}
      title={status.accessibleText}
      aria-label={status.accessibleText}
      data-testid="pollers-header-summary"
    >
      {status.visibleText}
    </span>
  );
}
