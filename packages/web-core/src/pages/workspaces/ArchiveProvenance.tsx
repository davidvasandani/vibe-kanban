import { useQuery } from '@tanstack/react-query';
import type { ArchiveIntent } from 'shared/types';
import { workspaceArchiveApi } from '@/shared/lib/api';

export function ArchiveProvenanceDetails({
  events,
  archived,
}: {
  events: ArchiveIntent[];
  archived: boolean;
}) {
  const lastArchive = events
    .filter((item) => item.archived_at !== null)
    .sort((a, b) => Date.parse(b.archived_at!) - Date.parse(a.archived_at!))[0];
  const event = archived
    ? (lastArchive ?? events[0])
    : (events.find((item) => !item.acknowledged) ?? lastArchive ?? events[0]);
  const showArchived = !!event?.archived_at && (archived || event.acknowledged);
  const occurredAt = showArchived ? event?.archived_at : event?.triggered_at;
  if (!event && !archived) return null;
  return (
    <div
      className="px-base py-half text-sm text-low"
      data-testid="archive-provenance"
    >
      {event ? (
        <>
          <span>
            {showArchived ? 'Archived by' : 'Archive requested by'} {event.rule}
          </span>{' '}
          <time dateTime={occurredAt ?? event.triggered_at}>
            {new Date(occurredAt ?? event.triggered_at).toLocaleString()}
          </time>
          .
          {event.acknowledged &&
            ' Explicitly unarchived; this event will not archive it again.'}
        </>
      ) : (
        'Archived before provenance was recorded; the rule is unknown.'
      )}
      {archived &&
        ' Unarchive to resume work. Scheduling a wake-up also reactivates an available worktree.'}
    </div>
  );
}

export function ArchiveProvenance({
  workspaceId,
  archived,
}: {
  workspaceId: string;
  archived: boolean;
}) {
  const { data, error } = useQuery({
    queryKey: ['workspace-archive-provenance', workspaceId],
    queryFn: ({ signal }) =>
      workspaceArchiveApi.provenance(workspaceId, signal),
    refetchInterval: 30000,
  });
  if (error)
    return (
      <p role="alert" className="px-base text-sm text-error">
        Unable to load archive reason: {error.message}
      </p>
    );
  if (!data) return null;
  return <ArchiveProvenanceDetails events={data} archived={archived} />;
}
