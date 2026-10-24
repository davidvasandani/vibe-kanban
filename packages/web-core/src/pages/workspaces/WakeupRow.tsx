import { useEffect, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '@vibe/ui/components/Button';
import type { WakeupSummary } from 'shared/types';
import { workspaceWakeupsApi } from '@/shared/lib/api';
import { workspaceWakeupsKey } from './useWorkspaceWakeups';

function countdown(fireAt: string, now: number) {
  const seconds = Math.ceil((Date.parse(fireAt) - now) / 1000);
  if (seconds <= 0) return 'Due now — awaiting delivery';
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m ${seconds % 60}s remaining`;
}

export function WakeupRow({ summary }: { summary: WakeupSummary }) {
  const { wakeup, delivery } = summary;
  const queryClient = useQueryClient();
  const [now, setNow] = useState(Date.now);
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pending = delivery.state === 'pending';
  useEffect(() => {
    if (!pending || wakeup.poller_id) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [pending, wakeup.poller_id]);

  async function cancel() {
    setCancelling(true);
    setError(null);
    try {
      if (!(await workspaceWakeupsApi.cancel(wakeup.workspace_id, wakeup.id))) {
        setError(
          'Wake-up could not be cancelled: delivery has already started or its state changed.'
        );
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setCancelling(false);
      void queryClient.invalidateQueries({
        queryKey: workspaceWakeupsKey(wakeup.workspace_id),
      });
    }
  }

  return (
    <li className="px-3 py-2 text-xs" data-testid="wakeup-row">
      <div className="flex items-center justify-between gap-2">
        <span>
          {wakeup.poller_id ? 'Poller completion wake-up' : 'Scheduled wake-up'}
        </span>
        {pending && (
          <Button
            variant="ghost"
            size="sm"
            onClick={cancel}
            disabled={cancelling}
            aria-label="Cancel wake-up"
          >
            {cancelling ? 'Cancelling…' : 'Cancel'}
          </Button>
        )}
      </div>
      <p className="text-low">{delivery.state}</p>
      {wakeup.poller_id ? (
        <p className="text-low">
          {pending
            ? 'Waiting for stop command to succeed'
            : 'Triggered by poller stop command'}
        </p>
      ) : (
        <p className="text-low">
          <time dateTime={wakeup.fire_at} title={wakeup.fire_at}>
            {new Date(wakeup.fire_at).toLocaleString()}
          </time>
          {pending && <span> · {countdown(wakeup.fire_at, now)}</span>}
        </p>
      )}
      <p
        className="whitespace-pre-wrap break-words"
        aria-label="Wake-up prompt"
      >
        {wakeup.prompt}
      </p>
      {delivery.reason && (
        <p className="text-low">{delivery.reason.replaceAll('_', ' ')}</p>
      )}
      {delivery.error && <p className="text-error">{delivery.error}</p>}
      {wakeup.trigger_context && (
        <p className="whitespace-pre-wrap break-words text-low">
          {wakeup.trigger_context}
        </p>
      )}
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
    </li>
  );
}
