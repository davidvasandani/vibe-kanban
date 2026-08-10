import { useEffect, useState } from 'react';

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

export function formatDeploymentAge(
  startedAt: string | null | undefined,
  nowMs = Date.now()
): string | null {
  if (!startedAt) return null;

  const startedAtMs = Date.parse(startedAt);
  if (!Number.isFinite(startedAtMs)) return null;

  const elapsedMs = Math.max(0, nowMs - startedAtMs);
  if (elapsedMs < MINUTE_MS) return 'now';
  if (elapsedMs < HOUR_MS) return `${Math.floor(elapsedMs / MINUTE_MS)}m`;
  if (elapsedMs < DAY_MS) return `${Math.floor(elapsedMs / HOUR_MS)}h`;
  return `${Math.floor(elapsedMs / DAY_MS)}d`;
}

interface MobileDeployStatusProps {
  revision?: string | null;
  startedAt?: string | null;
}

export function MobileDeployStatus({
  revision,
  startedAt,
}: MobileDeployStatusProps) {
  const [nowMs, setNowMs] = useState(Date.now);

  useEffect(() => {
    if (!startedAt) return;
    const timer = window.setInterval(() => setNowMs(Date.now()), MINUTE_MS);
    return () => window.clearInterval(timer);
  }, [startedAt]);

  const age = formatDeploymentAge(startedAt, nowMs);
  const hasRevision = Boolean(revision);

  if (!hasRevision && !age) return null;

  const revisionLabel = revision ? (
    revision === 'dev' ? (
      <span>{revision}</span>
    ) : (
      <a
        href={`https://github.com/davidvasandani/vibe-kanban/commit/${revision}`}
        target="_blank"
        rel="noopener noreferrer"
        className="hover:text-normal transition-colors"
        aria-label={`View deployed commit ${revision}`}
      >
        {revision}
      </a>
    )
  ) : null;

  return (
    <span
      className="flex shrink-0 items-center gap-0.5 whitespace-nowrap font-ibm-plex-mono text-[9px] leading-none text-low"
      title={
        revision && startedAt
          ? `Deployed ${revision} since ${startedAt}`
          : revision
            ? `Deployed ${revision}`
            : `Deployed since ${startedAt}`
      }
      aria-label={[
        revision && `Deployed commit ${revision}`,
        age && `${age} ago`,
      ]
        .filter(Boolean)
        .join(', ')}
    >
      {revisionLabel}
      {revisionLabel && age && <span aria-hidden="true">·</span>}
      {age && <span>{age}</span>}
    </span>
  );
}
