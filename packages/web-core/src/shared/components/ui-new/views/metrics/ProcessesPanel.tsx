import { useTranslation } from 'react-i18next';
import type { ProcessSample } from 'shared/types';

import { formatBytes, formatCount, formatPercent, NO_READING } from './format';
import { MetricsSection } from './MetricsSection';

export const PROCESSES_PANEL_ID = 'processes';

export interface ProcessesPanelProps {
  /** `null` means no process table was readable — not "no processes". */
  processes: readonly ProcessSample[] | null;
  expanded: boolean;
  onToggle: (panelId: string) => void;
  stale?: boolean;
}

/**
 * Top processes by CPU (FR-4).
 *
 * `command` arrives already redacted and truncated by the collector on the
 * host it was read from, so an unmasked credential cannot reach this view
 * (FR-26). Rows are observation only — nothing here can act on a process
 * (FR-23), so no row is a `button`.
 */
export function ProcessesPanel({
  processes,
  expanded,
  onToggle,
  stale,
}: ProcessesPanelProps) {
  const { t } = useTranslation('common');
  const title = t('metrics.processes.title', { defaultValue: 'Processes' });

  return (
    <MetricsSection
      panelId={PROCESSES_PANEL_ID}
      title={title}
      expanded={expanded}
      onToggle={onToggle}
      stale={stale}
      summary={
        <span className="font-ibm-plex-mono text-sm text-low tabular-nums">
          {processes === null ? '' : processes.length}
        </span>
      }
    >
      {processes === null ? (
        <p className="text-sm text-low">
          {t('metrics.processes.unreadable', {
            defaultValue: 'Process table unreadable',
          })}
        </p>
      ) : processes.length === 0 ? (
        <p className="text-sm text-low">
          {t('metrics.processes.none', {
            defaultValue: 'No processes reported',
          })}
        </p>
      ) : (
        processes.map((proc) => {
          const cpu = formatPercent(proc.cpu_percent);
          const memory = formatBytes(proc.memory_bytes);
          return (
            // `(pid, start_ticks)` is the only stable identity: a pid alone is
            // reused after a process exits, and an array index reorders on
            // every tick because the table is ranked by CPU.
            <div
              key={`${proc.pid}:${proc.start_ticks}`}
              data-testid="metrics-process"
              aria-label={t('metrics.processes.rowAria', {
                defaultValue:
                  '{{name}}, pid {{pid}}, CPU {{cpu}}, memory {{memory}}',
                name: proc.name,
                pid: proc.pid,
                cpu,
                memory,
              })}
              className="flex flex-col gap-half"
            >
              <div
                aria-hidden="true"
                className="flex items-baseline justify-between gap-half min-w-0"
              >
                <span className="text-normal truncate">{proc.name}</span>
                <span className="font-ibm-plex-mono text-sm text-low tabular-nums shrink-0">
                  {cpu}
                </span>
              </div>
              <div
                aria-hidden="true"
                className="flex items-baseline justify-between gap-half min-w-0"
              >
                <span className="font-ibm-plex-mono text-sm text-low truncate">
                  {proc.user ?? NO_READING}
                </span>
                <span className="font-ibm-plex-mono text-sm text-low tabular-nums shrink-0">
                  {t('metrics.processes.rowMeta', {
                    defaultValue: 'pid {{pid}} · {{memory}} · {{threads}} thr',
                    pid: proc.pid,
                    memory,
                    threads: formatCount(proc.thread_count),
                  })}
                </span>
              </div>
              <p
                aria-hidden="true"
                className="font-ibm-plex-mono text-sm text-low break-all"
              >
                {proc.command}
              </p>
            </div>
          );
        })
      )}
    </MetricsSection>
  );
}
