import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { create, useModal } from '@ebay/nice-modal-react';
import { SpinnerIcon } from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import type {
  ChngTicket,
  HashmalChngLookup,
  HashmalSession,
} from 'shared/types';
import { chngApi } from '@/shared/lib/api';
import {
  buildChngContinuationPrompt,
  buildChngFreshPrompt,
} from '@/shared/lib/chngPrompt';
import { formatRelativeTime } from '@/shared/lib/date';
import { defineModal } from '@/shared/lib/modals';
import { cn } from '@/shared/lib/utils';

export interface ChngStartSessionDialogProps {
  ticket: ChngTicket;
}

/** The first prompt for the new workspace, or null when canceled. */
export type ChngStartSessionResult = { prompt: string } | null;

function lookupNotice(lookup: HashmalChngLookup | undefined): string | null {
  if (!lookup || lookup.state === 'found') return null;
  return lookup.message ?? 'Hashmal has no session for this ticket.';
}

function SessionRow({
  session,
  isBest,
  manual,
  busy,
  disabled,
  onContinue,
}: {
  session: HashmalSession;
  isBest: boolean;
  manual: boolean;
  busy: boolean;
  disabled: boolean;
  onContinue: () => void;
}) {
  return (
    <li className="flex items-center gap-base py-half">
      <span
        className={cn(
          'size-2 shrink-0 rounded-full',
          session.running ? 'bg-success' : 'bg-low/40'
        )}
        title={session.running ? 'Running in Hashmal' : 'Not running'}
      />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm text-high" title={session.title}>
          {session.title}
        </p>
        <p className="truncate text-xs text-low">
          {isBest && (
            <span className="text-brand">
              {manual ? 'Linked in Hashmal · ' : 'Best match · '}
            </span>
          )}
          {session.last_active
            ? `active ${formatRelativeTime(session.last_active)}`
            : session.session_id}
        </p>
      </div>
      <Button
        size="sm"
        variant={isBest ? 'default' : 'outline'}
        onClick={onContinue}
        disabled={disabled}
      >
        {busy && <SpinnerIcon className="mr-1 size-3 animate-spin" />}
        Continue
      </Button>
    </li>
  );
}

const ChngStartSessionDialogImpl = create<ChngStartSessionDialogProps>(
  ({ ticket }) => {
    const modal = useModal();
    const [continuing, setContinuing] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    // Hashmal is asked first; its answer never blocks a fresh session.
    const lookup = useQuery({
      queryKey: ['chng', 'hashmal', ticket.key],
      queryFn: ({ signal }) => chngApi.lookupHashmal(ticket.key, null, signal),
      retry: false,
      staleTime: 0,
      gcTime: 0,
    });

    const finish = (result: ChngStartSessionResult) => {
      modal.resolve(result);
      // Removed, not just hidden: reopening must ask Hashmal again rather
      // than show the previous lookup.
      modal.hide();
      modal.remove();
    };

    const handleContinue = async (session: HashmalSession) => {
      setError(null);
      setContinuing(session.session_id);
      try {
        let transcript = lookup.data?.transcript ?? null;
        if (transcript?.session_id !== session.session_id) {
          const targeted = await chngApi.lookupHashmal(
            ticket.key,
            session.session_id
          );
          if (targeted.state !== 'found') {
            setError(
              targeted.message ?? 'Hashmal could not load that session.'
            );
            return;
          }
          transcript = targeted.transcript ?? null;
        }
        finish({
          prompt: buildChngContinuationPrompt(ticket, session, transcript),
        });
      } catch (err) {
        setError(
          err instanceof Error
            ? `Hashmal could not load that session: ${err.message}`
            : 'Hashmal could not load that session.'
        );
      } finally {
        setContinuing(null);
      }
    };

    const sessions = lookup.data?.state === 'found' ? lookup.data.sessions : [];
    const notice = lookup.isError
      ? `Couldn't reach Hashmal: ${
          lookup.error instanceof Error ? lookup.error.message : 'unknown error'
        }`
      : lookupNotice(lookup.data);

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => {
          if (!open) finish(null);
        }}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Start a session for {ticket.key}</DialogTitle>
            <DialogDescription className="truncate" title={ticket.summary}>
              {ticket.summary}
            </DialogDescription>
          </DialogHeader>

          {lookup.isPending ? (
            <p className="flex items-center gap-half text-sm text-low">
              <SpinnerIcon className="size-icon-sm animate-spin" />
              Looking in Hashmal for the session that worked this ticket…
            </p>
          ) : sessions.length > 0 ? (
            <div>
              <p className="text-sm text-normal">
                Hashmal found{' '}
                {sessions.length === 1
                  ? 'a session'
                  : `${sessions.length} sessions`}{' '}
                for this ticket. Continue one to start a new session with its
                recent transcript as context.
              </p>
              <ul className="mt-half max-h-64 overflow-y-auto divide-y divide-border">
                {sessions.map((session, index) => (
                  <SessionRow
                    key={session.session_id}
                    session={session}
                    isBest={index === 0}
                    manual={lookup.data?.manual ?? false}
                    busy={continuing === session.session_id}
                    disabled={continuing !== null}
                    onContinue={() => void handleContinue(session)}
                  />
                ))}
              </ul>
            </div>
          ) : (
            notice && <p className="text-sm text-low">{notice}</p>
          )}

          {error && <p className="text-sm text-error">{error}</p>}

          <DialogFooter>
            <Button variant="outline" onClick={() => finish(null)}>
              Cancel
            </Button>
            <Button
              variant={sessions.length > 0 ? 'outline' : 'default'}
              onClick={() => finish({ prompt: buildChngFreshPrompt(ticket) })}
              disabled={continuing !== null}
            >
              Start a new session
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const ChngStartSessionDialog = defineModal<
  ChngStartSessionDialogProps,
  ChngStartSessionResult
>(ChngStartSessionDialogImpl);
