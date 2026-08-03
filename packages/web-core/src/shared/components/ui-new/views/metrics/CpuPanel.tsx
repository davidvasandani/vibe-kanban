import { useTranslation } from 'react-i18next';
import type { CpuSample } from 'shared/types';
import { Meter } from '@vibe/ui/components/Meter';
import { Sparkline } from '@vibe/ui/components/Sparkline';

import { formatCount, formatLoad, formatPercent, NO_READING } from './format';
import { MetricsRow, MetricsSection } from './MetricsSection';

export const CPU_PANEL_ID = 'cpu';

export interface CpuPanelProps {
  cpu: CpuSample | null;
  /** Total-busy percentage per retained sample, oldest first. */
  history: readonly (number | null)[];
  expanded: boolean;
  onToggle: (panelId: string) => void;
  stale?: boolean;
}

/**
 * Overall and per-core CPU, load averages, and a short history graph (FR-4,
 * FR-12).
 *
 * Every reading is `number | null`; a `null` is rendered as an em dash by
 * `Meter`/`formatPercent`, never as `0` — before the first delta exists there
 * is no utilisation to report, and reporting it as idle would be a lie.
 */
export function CpuPanel({
  cpu,
  history,
  expanded,
  onToggle,
  stale,
}: CpuPanelProps) {
  const { t } = useTranslation('common');
  const title = t('metrics.cpu.title', { defaultValue: 'CPU' });
  const total = cpu?.total_busy_percent ?? null;
  const perCore = cpu?.per_core_busy_percent ?? null;

  return (
    <MetricsSection
      panelId={CPU_PANEL_ID}
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
          valueText={formatPercent(total)}
        />
      }
    >
      <Meter
        label={t('metrics.cpu.total', { defaultValue: 'Total' })}
        value={total}
        valueText={formatPercent(total)}
      />
      <MetricsRow
        label={t('metrics.cpu.model', { defaultValue: 'Model' })}
        value={cpu?.model ?? NO_READING}
      />
      <MetricsRow
        label={t('metrics.cpu.cores', { defaultValue: 'Cores' })}
        value={formatCount(cpu?.core_count)}
      />
      <MetricsRow
        label={t('metrics.cpu.load', { defaultValue: 'Load 1m / 5m / 15m' })}
        value={`${formatLoad(cpu?.load_1m)} / ${formatLoad(
          cpu?.load_5m
        )} / ${formatLoad(cpu?.load_15m)}`}
      />
      <MetricsRow
        label={t('metrics.cpu.frequency', { defaultValue: 'Frequency' })}
        value={
          cpu?.frequency_mhz === null || cpu?.frequency_mhz === undefined
            ? NO_READING
            : `${Math.round(cpu.frequency_mhz)} MHz`
        }
      />
      <MetricsRow
        label={t('metrics.cpu.temperature', { defaultValue: 'Temperature' })}
        value={
          cpu?.temperature_celsius === null ||
          cpu?.temperature_celsius === undefined
            ? NO_READING
            : `${cpu.temperature_celsius.toFixed(1)} °C`
        }
      />
      {perCore === null ? (
        <p className="text-sm text-low">
          {t('metrics.cpu.noPerCore', {
            defaultValue: 'Per-core readings unavailable',
          })}
        </p>
      ) : (
        <div
          data-testid="metrics-cpu-cores"
          className="grid grid-cols-2 gap-half"
        >
          {/*
            Core index is the identity here — core 3 is core 3 — so the
            position *is* the key, unlike every other list in this drawer.
          */}
          {perCore.map((value, index) => (
            <Meter
              key={index}
              label={t('metrics.cpu.core', {
                defaultValue: 'Core {{index}}',
                index,
              })}
              value={value}
              valueText={formatPercent(value, 0)}
            />
          ))}
        </div>
      )}
    </MetricsSection>
  );
}
