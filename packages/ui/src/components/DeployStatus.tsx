import { useEffect, useState } from 'react';
import { cn } from '../lib/cn';

const UPDATE_INTERVAL_MS = 60_000;

export interface DeployAge {
  compact: string;
  description: string;
}

export function formatDeployAge(
  timestamp: string,
  nowMs = Date.now()
): DeployAge | null {
  const deployedAtMs = Date.parse(timestamp);
  if (!Number.isFinite(deployedAtMs)) return null;

  const elapsedMinutes = Math.max(
    0,
    Math.floor((nowMs - deployedAtMs) / 60_000)
  );

  if (elapsedMinutes < 1) {
    return { compact: 'now', description: 'less than a minute ago' };
  }
  if (elapsedMinutes < 60) {
    return {
      compact: `${elapsedMinutes}m`,
      description: `${elapsedMinutes} minute${elapsedMinutes === 1 ? '' : 's'} ago`,
    };
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return {
      compact: `${elapsedHours}h`,
      description: `${elapsedHours} hour${elapsedHours === 1 ? '' : 's'} ago`,
    };
  }

  const elapsedDays = Math.floor(elapsedHours / 24);
  if (elapsedDays < 7) {
    return {
      compact: `${elapsedDays}d`,
      description: `${elapsedDays} day${elapsedDays === 1 ? '' : 's'} ago`,
    };
  }

  const elapsedWeeks = Math.floor(elapsedDays / 7);
  return {
    compact: `${elapsedWeeks}w`,
    description: `${elapsedWeeks} week${elapsedWeeks === 1 ? '' : 's'} ago`,
  };
}

export interface DeployStatusProps {
  version: string | null;
  deploymentTimestamp?: string | null;
  className?: string;
  alwaysShowAge?: boolean;
}

export function DeployStatus({
  version,
  deploymentTimestamp,
  className,
  alwaysShowAge = false,
}: DeployStatusProps) {
  const [nowMs, setNowMs] = useState(Date.now());

  useEffect(() => {
    if (!deploymentTimestamp) return;
    const interval = window.setInterval(
      () => setNowMs(Date.now()),
      UPDATE_INTERVAL_MS
    );
    return () => window.clearInterval(interval);
  }, [deploymentTimestamp]);

  if (!version) return null;

  const age = deploymentTimestamp
    ? formatDeployAge(deploymentTimestamp, nowMs)
    : null;
  const label =
    version === 'dev'
      ? 'Development build'
      : `Deployed revision ${version}${age ? ` ${age.description}` : ''}`;
  const content = (
    <>
      <span className="truncate">{version}</span>
      {version !== 'dev' && age && (
        <span
          className={cn(
            'shrink-0',
            alwaysShowAge ? 'inline' : 'hidden min-[390px]:inline'
          )}
        >
          {' '}
          · {age.compact}
        </span>
      )}
    </>
  );
  const classes = cn(
    'flex min-w-0 max-w-20 items-center font-ibm-plex-mono text-[9px] leading-none text-low',
    className
  );

  return version === 'dev' ? (
    <span className={classes} aria-label={label} title={label}>
      {content}
    </span>
  ) : (
    <a
      href={`https://github.com/davidvasandani/vibe-kanban/commit/${version}`}
      target="_blank"
      rel="noopener noreferrer"
      className={cn(classes, 'hover:text-normal transition-colors')}
      aria-label={label}
      title={label}
    >
      {content}
    </a>
  );
}
