import { useTranslation } from 'react-i18next';

import { cn } from '../lib/cn';

/**
 * Severity buckets shared by the metrics primitives (`Meter`, `Sparkline`).
 *
 * The thresholds and the design tokens they map to deliberately mirror
 * `ContextUsageGauge` so every gauge in the app escalates the same way. This
 * is *not* btop's raw ANSI palette — severity is expressed with the design
 * system's text tokens only.
 */
export type MetricSeverity = 'low' | 'medium' | 'high' | 'critical';

/** Maps a 0..1 fill ratio onto a severity bucket. */
export function severityForRatio(ratio: number): MetricSeverity {
  const pct = ratio * 100;
  if (pct < 50) return 'low';
  if (pct < 75) return 'medium';
  if (pct < 90) return 'high';
  return 'critical';
}

/** Design-token text colour for each severity bucket. */
export const SEVERITY_TEXT_CLASS: Record<MetricSeverity, string> = {
  low: 'text-low',
  medium: 'text-normal',
  high: 'text-brand-secondary',
  critical: 'text-error',
};

/** Rendered in place of a reading that does not exist. */
export const NO_READING = '—';

export function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export interface MeterProps {
  /** Already-translated caption for the reading, e.g. "CPU". */
  label: string;
  /**
   * The reading. `null` means *no reading was taken* and renders as an em
   * dash — never as an empty or zero-width bar, which would read as "0".
   */
  value: number | null;
  /** Lower bound of the meter's domain. Defaults to `0`. */
  min?: number;
  /** Upper bound of the meter's domain. Defaults to `100`. */
  max?: number;
  /**
   * Pre-formatted text for the reading, e.g. `"42%"` or `"1.2 GiB"`. Falls
   * back to the raw number when omitted.
   */
  valueText?: string;
  /** Force a severity bucket instead of deriving it from the fill ratio. */
  severity?: MetricSeverity;
  /** Render the caption visually. The aria label always states it. */
  hideLabel?: boolean;
  className?: string;
}

/**
 * A stateless proportional bar with a monospace numeric readout (FR-12).
 *
 * Hand-rolled Tailwind width bar — no charting library (research R7).
 * Carries `role="img"` with a value-stating `aria-label` so the reading has a
 * text equivalent for screen readers (FR-14).
 */
export function Meter({
  label,
  value,
  min = 0,
  max = 100,
  valueText,
  severity,
  hideLabel = false,
  className,
}: MeterProps) {
  const { t } = useTranslation('common');

  const missing = value === null || !Number.isFinite(value);
  const span = max - min;
  const ratio =
    missing || span <= 0 ? 0 : clamp(((value as number) - min) / span, 0, 1);
  const bucket = severity ?? severityForRatio(ratio);

  const readout = missing
    ? NO_READING
    : (valueText ?? String(Math.round((value as number) * 100) / 100));

  const ariaLabel = t('metrics.meterAria', {
    defaultValue: '{{label}}: {{value}}',
    label,
    value: missing
      ? t('metrics.noReading', { defaultValue: 'no reading' })
      : readout,
  });

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      className={cn('flex items-center gap-half min-w-0', className)}
    >
      {!hideLabel && (
        <span aria-hidden="true" className="text-sm text-low truncate shrink-0">
          {label}
        </span>
      )}
      {missing ? (
        <span
          aria-hidden="true"
          data-testid="meter-no-reading"
          className="flex-1 font-ibm-plex-mono text-base text-low"
        >
          {NO_READING}
        </span>
      ) : (
        <>
          <div
            aria-hidden="true"
            data-testid="meter-track"
            className="flex-1 h-half min-w-0 rounded-sm bg-panel overflow-hidden"
          >
            <div
              data-testid="meter-fill"
              className={cn(
                'h-full rounded-sm bg-current transition-all duration-300 ease-out',
                SEVERITY_TEXT_CLASS[bucket]
              )}
              style={{ width: `${ratio * 100}%` }}
            />
          </div>
          <span
            aria-hidden="true"
            className={cn(
              'font-ibm-plex-mono text-sm tabular-nums shrink-0',
              SEVERITY_TEXT_CLASS[bucket]
            )}
          >
            {readout}
          </span>
        </>
      )}
    </div>
  );
}
