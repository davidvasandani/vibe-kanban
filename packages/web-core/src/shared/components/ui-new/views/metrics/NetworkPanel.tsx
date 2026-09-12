import { useTranslation } from 'react-i18next';
import type { NetworkSample } from 'shared/types';

import { formatBytes, formatBytesPerSecond } from './format';
import { MetricsRow, MetricsSection } from './MetricsSection';

export const NETWORK_PANEL_ID = 'network';

export interface NetworkPanelProps {
  /** `null` means `/proc/net/dev` was unreadable — not "no interfaces". */
  networks: readonly NetworkSample[] | null;
  expanded: boolean;
  onToggle: (panelId: string) => void;
  stale?: boolean;
}

/**
 * Per-interface throughput and lifetime totals (FR-4).
 *
 * Throughput is rate-derived, so it is `null` on the first sample and after a
 * counter reset (FR-7). That renders as an em dash: a `0 B/s` would read as
 * "no traffic", which is a different and false claim.
 */
export function NetworkPanel({
  networks,
  expanded,
  onToggle,
  stale,
}: NetworkPanelProps) {
  const { t } = useTranslation('common');
  const title = t('metrics.network.title', { defaultValue: 'Network' });

  return (
    <MetricsSection
      panelId={NETWORK_PANEL_ID}
      title={title}
      expanded={expanded}
      onToggle={onToggle}
      stale={stale}
      summary={
        <span className="font-ibm-plex-mono text-sm text-low tabular-nums">
          {networks === null ? '' : networks.length}
        </span>
      }
    >
      {networks === null ? (
        <p className="text-sm text-low">
          {t('metrics.network.unreadable', {
            defaultValue: 'Interface counters unreadable',
          })}
        </p>
      ) : networks.length === 0 ? (
        <p className="text-sm text-low">
          {t('metrics.network.none', {
            defaultValue: 'No interfaces reported',
          })}
        </p>
      ) : (
        networks.map((net) => (
          // Interface name is the identity; index shifts when one appears.
          <div
            key={net.interface}
            data-testid="metrics-interface"
            className="flex flex-col gap-half"
          >
            <MetricsRow
              label={net.interface}
              value={`${formatBytesPerSecond(
                net.rx_bytes_per_second
              )} ↓ / ${formatBytesPerSecond(net.tx_bytes_per_second)} ↑`}
            />
            <MetricsRow
              label={t('metrics.network.total', { defaultValue: 'Total' })}
              value={`${formatBytes(net.rx_bytes_total)} ↓ / ${formatBytes(
                net.tx_bytes_total
              )} ↑`}
            />
          </div>
        ))
      )}
    </MetricsSection>
  );
}
