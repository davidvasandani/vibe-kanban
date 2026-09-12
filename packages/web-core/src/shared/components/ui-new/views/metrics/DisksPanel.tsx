import { useTranslation } from 'react-i18next';
import type { FilesystemSample } from 'shared/types';
import { Meter } from '@vibe/ui/components/Meter';

import { formatBytes, formatPercent, ratioPercent } from './format';
import { MetricsRow, MetricsSection } from './MetricsSection';

export const DISKS_PANEL_ID = 'disks';

export interface DisksPanelProps {
  /** `null` means the mount table was unreadable — not "no filesystems". */
  filesystems: readonly FilesystemSample[] | null;
  expanded: boolean;
  onToggle: (panelId: string) => void;
  stale?: boolean;
}

/**
 * Per-filesystem usage, including the shared NFS mount (FR-4).
 *
 * A mount whose `statvfs` failed reports `null` totals: a stalled NFS server
 * is the common case, and rendering it as `0 B` would be a lie about the one
 * filesystem this panel most exists to watch.
 */
export function DisksPanel({
  filesystems,
  expanded,
  onToggle,
  stale,
}: DisksPanelProps) {
  const { t } = useTranslation('common');
  const title = t('metrics.disks.title', { defaultValue: 'Filesystems' });

  return (
    <MetricsSection
      panelId={DISKS_PANEL_ID}
      title={title}
      expanded={expanded}
      onToggle={onToggle}
      stale={stale}
      summary={
        <span className="font-ibm-plex-mono text-sm text-low tabular-nums">
          {filesystems === null ? '' : filesystems.length}
        </span>
      }
    >
      {filesystems === null ? (
        <p className="text-sm text-low">
          {t('metrics.disks.unreadable', {
            defaultValue: 'Mount table unreadable',
          })}
        </p>
      ) : filesystems.length === 0 ? (
        <p className="text-sm text-low">
          {t('metrics.disks.none', { defaultValue: 'No filesystems reported' })}
        </p>
      ) : (
        filesystems.map((fs) => {
          const usedPercent = ratioPercent(fs.used_bytes, fs.total_bytes);
          return (
            // Mount point is the identity: a device can be mounted twice and
            // an index shifts whenever a mount appears or goes away.
            <div
              key={`${fs.mount_point}:${fs.device}`}
              data-testid="metrics-filesystem"
              className="flex flex-col gap-half"
            >
              <MetricsRow label={fs.mount_point} value={fs.fs_type} />
              <Meter
                label={fs.mount_point}
                value={usedPercent}
                valueText={formatPercent(usedPercent)}
                hideLabel
              />
              <MetricsRow
                label={t('metrics.disks.usage', { defaultValue: 'Used' })}
                value={`${formatBytes(fs.used_bytes)} / ${formatBytes(
                  fs.total_bytes
                )}`}
              />
              <MetricsRow
                label={t('metrics.disks.available', {
                  defaultValue: 'Available',
                })}
                value={formatBytes(fs.available_bytes)}
              />
            </div>
          );
        })
      )}
    </MetricsSection>
  );
}
