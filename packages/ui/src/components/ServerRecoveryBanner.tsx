import { useEffect, useState } from 'react';
import { WarningIcon, XIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from './PrimaryButton';

export type ServerRecoveryPhase =
  | 'suggesting'
  | 'waiting'
  | 'moving'
  | 'resending'
  | 'expired';

export interface ServerRecoveryNoticeProps {
  phase: ServerRecoveryPhase;
  /** Hostname of the execution server that refused the send. */
  server: string;
  /** The server's explanation (pressure, draining, offline). */
  message: string;
  /** Servers that can continue the session, best first. */
  alternatives: { id: string; label: string }[];
  /** Epoch ms of the next automatic check, while one is scheduled. */
  nextCheckAt: number | null;
  /** Epoch ms when automatic checks stop. */
  deadline: number;
  error?: string | null;
  onMoveTo: (id: string) => void;
  onRetryNow: () => void;
  onDismiss: () => void;
}

const MAX_SUGGESTIONS = 3;

function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const interval = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [active]);
  return now;
}

/**
 * Shown when the session's execution server refused a message: suggests
 * another server, or explains that Vibe Kanban is waiting and retrying.
 */
export function ServerRecoveryBanner({
  phase,
  server,
  message,
  alternatives,
  nextCheckAt,
  deadline,
  error,
  onMoveTo,
  onRetryNow,
  onDismiss,
}: ServerRecoveryNoticeProps) {
  const { t } = useTranslation('tasks');
  const counting = phase === 'suggesting' || phase === 'waiting';
  const now = useNow(counting);
  const busy = phase === 'moving' || phase === 'resending';
  const minutes = Math.max(1, Math.ceil((deadline - now) / 60_000));
  const seconds =
    nextCheckAt === null
      ? null
      : Math.max(0, Math.ceil((nextCheckAt - now) / 1000));

  const headline = (() => {
    switch (phase) {
      case 'suggesting':
        return t('conversation.serverRecovery.suggesting', { server });
      case 'waiting':
        return t('conversation.serverRecovery.waiting', { server, minutes });
      case 'moving':
        return t('conversation.serverRecovery.moving');
      case 'resending':
        return t('conversation.serverRecovery.resending');
      case 'expired':
        return t('conversation.serverRecovery.expired', { server });
    }
  })();

  return (
    <div
      role="status"
      className="bg-warning/5 border-b px-double py-base flex flex-col gap-half"
    >
      <div className="flex items-start gap-base">
        <WarningIcon className="h-4 w-4 text-warning flex-shrink-0 mt-[2px]" />
        <div className="flex-1 min-w-0 flex flex-col gap-half">
          <span className="text-sm text-normal">{headline}</span>
          <span className="text-xs text-low break-words">{message}</span>
          {counting && seconds !== null && (
            <span className="text-xs text-low">
              {t('conversation.serverRecovery.nextCheck', { seconds })}
            </span>
          )}
          {error && (
            <span className="text-xs text-error break-words">{error}</span>
          )}
        </div>
        <button
          onClick={onDismiss}
          className="text-low hover:text-normal transition-colors p-1 -m-1"
          title={t('conversation.serverRecovery.dismiss')}
          aria-label={t('conversation.serverRecovery.dismiss')}
        >
          <XIcon className="h-4 w-4" />
        </button>
      </div>
      {(alternatives.length > 0 || phase === 'expired' || busy) && (
        <div className="flex flex-wrap items-center gap-half pl-double">
          {alternatives.slice(0, MAX_SUGGESTIONS).map((alternative) => (
            <PrimaryButton
              key={alternative.id}
              variant="secondary"
              value={t('conversation.serverRecovery.moveTo', {
                server: alternative.label,
              })}
              onClick={() => onMoveTo(alternative.id)}
              disabled={busy}
            />
          ))}
          {(phase === 'expired' || busy) && (
            <PrimaryButton
              variant="tertiary"
              value={t('conversation.serverRecovery.retryNow', { server })}
              onClick={onRetryNow}
              disabled={busy}
              actionIcon={busy ? 'spinner' : undefined}
            />
          )}
        </div>
      )}
    </div>
  );
}
