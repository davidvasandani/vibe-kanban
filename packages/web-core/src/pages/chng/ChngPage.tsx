import { useCallback } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  ArrowClockwiseIcon,
  ClipboardTextIcon,
  GearIcon,
  PlayIcon,
  SpinnerIcon,
} from '@phosphor-icons/react';
import type { ChngTicket } from 'shared/types';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { chngApi } from '@/shared/lib/api';
import { formatRelativeTime } from '@/shared/lib/date';
import { cn } from '@/shared/lib/utils';
import { setCreateModeSeedState } from '@/shared/stores/createModeSeedStore';
import { ChngStartSessionDialog } from './ChngStartSessionDialog';

export const CHNG_TICKETS_QUERY_KEY = ['chng', 'tickets'] as const;

function statusClassName(category: string): string {
  if (category === 'done') return 'bg-success/15 text-success';
  if (category === 'indeterminate') return 'bg-brand/15 text-brand';
  return 'bg-secondary text-normal';
}

function TicketRow({
  ticket,
  onStart,
}: {
  ticket: ChngTicket;
  onStart: (ticket: ChngTicket) => void;
}) {
  return (
    <li className="flex items-center gap-base px-double py-base hover:bg-secondary">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-half">
          <a
            href={ticket.url}
            target="_blank"
            rel="noopener noreferrer"
            className="font-ibm-plex-mono text-sm text-high hover:underline"
          >
            {ticket.key}
          </a>
          {ticket.status && (
            <span
              className={cn(
                'rounded-sm px-half text-xs',
                statusClassName(ticket.status_category)
              )}
            >
              {ticket.status}
            </span>
          )}
        </div>
        <p className="truncate text-sm text-normal" title={ticket.summary}>
          {ticket.summary || '(no summary)'}
        </p>
      </div>
      {ticket.updated && (
        <span className="hidden shrink-0 text-xs text-low sm:inline">
          {formatRelativeTime(ticket.updated)}
        </span>
      )}
      <button
        type="button"
        onClick={() => onStart(ticket)}
        className="flex shrink-0 items-center gap-half rounded-sm border border-border px-base py-half text-sm text-normal hover:bg-tertiary cursor-pointer"
      >
        <PlayIcon className="size-icon-xs" weight="fill" />
        Start session
      </button>
    </li>
  );
}

export function ChngPage() {
  const appNavigation = useAppNavigation();
  const tickets = useQuery({
    queryKey: CHNG_TICKETS_QUERY_KEY,
    queryFn: ({ signal }) => chngApi.listTickets(signal),
    retry: false,
    refetchOnWindowFocus: true,
  });

  const openSettings = useCallback(() => {
    void SettingsDialog.show({ initialSection: 'chng' });
  }, []);

  const handleStart = useCallback(
    async (ticket: ChngTicket) => {
      const result = await ChngStartSessionDialog.show({ ticket });
      if (!result) return;
      setCreateModeSeedState({ initialPrompt: result.prompt });
      appNavigation.goToWorkspacesCreate();
    },
    [appNavigation]
  );

  const data = tickets.data;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center justify-between gap-base border-b border-border px-double py-base">
        <div className="min-w-0">
          <h1 className="text-xl font-medium text-high">CHNG tickets</h1>
          {data?.configured && (
            <p
              className="truncate font-ibm-plex-mono text-xs text-low"
              title={data.jql}
            >
              {data.jql}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-half">
          <button
            type="button"
            onClick={() => void tickets.refetch()}
            disabled={tickets.isFetching}
            aria-label="Refresh CHNG tickets"
            className="flex items-center gap-1 px-base py-half text-sm text-low hover:text-normal cursor-pointer disabled:cursor-default disabled:opacity-60"
          >
            <ArrowClockwiseIcon
              size={16}
              className={cn(tickets.isFetching && 'animate-spin')}
            />
            Refresh
          </button>
          <button
            type="button"
            onClick={openSettings}
            aria-label="CHNG settings"
            className="flex items-center gap-1 px-base py-half text-sm text-low hover:text-normal cursor-pointer"
          >
            <GearIcon size={16} />
            Settings
          </button>
        </div>
      </div>

      {tickets.isError && (
        <p className="border-b border-border px-double py-half text-sm text-error">
          {tickets.error instanceof Error
            ? tickets.error.message
            : 'Could not load CHNG tickets.'}
          {data ? ' Showing the last loaded list.' : ''}
        </p>
      )}

      <div className="flex-1 overflow-y-auto">
        {tickets.isPending ? (
          <div className="flex h-full items-center justify-center gap-half text-low">
            <SpinnerIcon className="size-icon-sm animate-spin" />
            Loading CHNG tickets…
          </div>
        ) : data && !data.configured ? (
          <div className="flex h-full flex-col items-center justify-center gap-base text-low">
            <ClipboardTextIcon size={32} weight="light" />
            <p className="text-base">
              Connect Jira in Settings to list your CHNG tickets.
            </p>
            <button
              type="button"
              onClick={openSettings}
              className="rounded-sm border border-border px-base py-half text-sm text-normal hover:bg-tertiary cursor-pointer"
            >
              Open CHNG settings
            </button>
          </div>
        ) : data && data.tickets.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-low">
            <ClipboardTextIcon size={32} weight="light" />
            <p className="text-base">No tickets match the CHNG search</p>
          </div>
        ) : data ? (
          <>
            <ul className="divide-y divide-border">
              {data.tickets.map((ticket) => (
                <TicketRow
                  key={ticket.key}
                  ticket={ticket}
                  onStart={(t) => void handleStart(t)}
                />
              ))}
            </ul>
            {data.truncated && (
              <p className="px-double py-base text-xs text-low">
                Showing the first {data.tickets.length} tickets. Narrow the
                search in Settings → CHNG to see the rest.
              </p>
            )}
          </>
        ) : null}
      </div>
    </div>
  );
}
