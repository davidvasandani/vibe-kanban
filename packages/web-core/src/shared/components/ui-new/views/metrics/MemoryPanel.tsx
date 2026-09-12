import { useTranslation } from 'react-i18next';
import type { MemorySample } from 'shared/types';
import { Meter } from '@vibe/ui/components/Meter';
import { Sparkline } from '@vibe/ui/components/Sparkline';

import { formatBytes, formatPercent, ratioPercent } from './format';
import { MetricsRow, MetricsSection } from './MetricsSection';

export const MEMORY_PANEL_ID = 'memory';

export interface MemoryPanelProps {
  memory: MemorySample | null;
  /** Used-memory percentage per retained sample, oldest first. */
  history: readonly (number | null)[];
  expanded: boolean;
  onToggle: (panelId: string) => void;
  stale?: boolean;
}

/**
 * Total / used / available / cached memory and swap (FR-4).
 *
 * `used` is `total − available`, computed on the host; an unreadable
 * `/proc/meminfo` yields `null` on every field and renders as em dashes
 * rather than a machine that appears to have no memory.
 */
export function MemoryPanel({
  memory,
  history,
  expanded,
  onToggle,
  stale,
}: MemoryPanelProps) {
  const { t } = useTranslation('common');
  const title = t('metrics.memory.title', { defaultValue: 'Memory' });

  const usedPercent = ratioPercent(memory?.used_bytes, memory?.total_bytes);
  const swapPercent = ratioPercent(
    memory?.swap_used_bytes,
    memory?.swap_total_bytes
  );

  return (
    <MetricsSection
      panelId={MEMORY_PANEL_ID}
      title={title}
      expanded={expanded}
      onToggle={onToggle}
      stale={stale}
      summary={
        <Sparkline
          label={title}
          values={history}
          min={0}
          max={100}
          valueText={formatPercent(usedPercent)}
        />
      }
    >
      <Meter
        label={t('metrics.memory.used', { defaultValue: 'Used' })}
        value={usedPercent}
        valueText={formatPercent(usedPercent)}
      />
      <MetricsRow
        label={t('metrics.memory.total', { defaultValue: 'Total' })}
        value={formatBytes(memory?.total_bytes)}
      />
      <MetricsRow
        label={t('metrics.memory.usedBytes', { defaultValue: 'Used bytes' })}
        value={formatBytes(memory?.used_bytes)}
      />
      <MetricsRow
        label={t('metrics.memory.available', { defaultValue: 'Available' })}
        value={formatBytes(memory?.available_bytes)}
      />
      <MetricsRow
        label={t('metrics.memory.cached', { defaultValue: 'Cached' })}
        value={formatBytes(memory?.cached_bytes)}
      />
      <Meter
        label={t('metrics.memory.swap', { defaultValue: 'Swap' })}
        value={swapPercent}
        valueText={formatPercent(swapPercent)}
      />
      <MetricsRow
        label={t('metrics.memory.swapUsed', { defaultValue: 'Swap used' })}
        value={`${formatBytes(memory?.swap_used_bytes)} / ${formatBytes(
          memory?.swap_total_bytes
        )}`}
      />
    </MetricsSection>
  );
}
