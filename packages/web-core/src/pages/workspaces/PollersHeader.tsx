import { useMemo } from 'react';
import type { ExecutionProcess, WakeupSummary } from 'shared/types';
import {
  derivePollersHeaderStatus,
  selectPollers,
} from '@/pages/workspaces/pollers';

interface PollersHeaderProps {
  executionProcesses: ExecutionProcess[];
  wakeups?: WakeupSummary[];
  wakeupsError?: Error | null;
}

/**
 * Collapsed-state summary for the Pollers section.
 *
 * Renders inside the section header row, which survives collapse, so the count
 * stays current while the body is unmounted. It takes the already-streamed
 * execution processes as a prop and issues no request of its own.
 */
export function PollersHeader({
  executionProcesses,
  wakeups = [],
  wakeupsError,
}: PollersHeaderProps) {
  const status = useMemo(
    () => derivePollersHeaderStatus(selectPollers(executionProcesses)),
    [executionProcesses]
  );

  const pending = wakeups.filter(
    ({ delivery }) => delivery.state === 'pending'
  ).length;
  const wakeupStatus = pending
    ? `${pending} pending wake-up${pending === 1 ? '' : 's'}`
    : '';
  const accessibleText = [
    status?.accessibleText,
    wakeupStatus,
    wakeupsError && 'Wake-ups unavailable',
  ]
    .filter(Boolean)
    .join(' · ');
  const visibleText = [
    status?.visibleText,
    wakeupStatus,
    wakeupsError && 'Wake-ups unavailable',
  ]
    .filter(Boolean)
    .join(' · ');
  if (!accessibleText) return null;

  return (
    <span
      className={`min-w-0 max-w-28 truncate text-sm ${
        status?.hasFailure || wakeupsError ? 'text-destructive' : 'text-low'
      }`}
      title={accessibleText}
      aria-label={accessibleText}
      data-testid="pollers-header-summary"
    >
      {visibleText}
    </span>
  );
}
