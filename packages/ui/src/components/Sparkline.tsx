import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { cn } from '../lib/cn';
import {
  clamp,
  NO_READING,
  SEVERITY_TEXT_CLASS,
  severityForRatio,
  type MetricSeverity,
} from './Meter';

export interface SparklineProps {
  /** Already-translated caption for the series, e.g. "CPU". */
  label: string;
  /**
   * The series, oldest first. A `null` entry is a *missing reading* and is
   * rendered as a gap in the line — it is never plotted as `0`, which would
   * be indistinguishable from a genuine zero reading.
   */
  values: readonly (number | null)[];
  /** Lower bound of the value domain. Defaults to `0`. */
  min?: number;
  /** Upper bound of the value domain. Defaults to the largest reading. */
  max?: number;
  /** SVG user-space width. Defaults to `96`. */
  width?: number;
  /** SVG user-space height. Defaults to `24`. */
  height?: number;
  /** SVG stroke width. Defaults to `1.5`. */
  strokeWidth?: number;
  /** Pre-formatted text for the latest reading, e.g. `"42%"`. */
  valueText?: string;
  /** Force a severity bucket instead of deriving it from the latest reading. */
  severity?: MetricSeverity;
  className?: string;
}

/** A run of consecutive non-null readings, in SVG user-space coordinates. */
interface Segment {
  points: Array<{ x: number; y: number }>;
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}

/**
 * Splits a series into runs of consecutive readings, dropping the nulls. Each
 * run becomes its own polyline, which is what produces the visible gaps.
 */
export function buildSparklineSegments(
  values: readonly (number | null)[],
  opts: {
    min: number;
    max: number;
    width: number;
    height: number;
    strokeWidth: number;
  }
): Segment[] {
  const { min, max, width, height, strokeWidth } = opts;
  const n = values.length;
  const span = max - min;
  const pad = strokeWidth / 2;
  const usableHeight = Math.max(0, height - strokeWidth);

  const x = (i: number) => (n <= 1 ? width / 2 : (i / (n - 1)) * width);
  const y = (value: number) => {
    const t = span <= 0 ? 0 : clamp((value - min) / span, 0, 1);
    return pad + (1 - t) * usableHeight;
  };

  const segments: Segment[] = [];
  let current: Segment | null = null;

  values.forEach((value, i) => {
    if (value === null || !Number.isFinite(value)) {
      current = null;
      return;
    }
    if (!current) {
      current = { points: [] };
      segments.push(current);
    }
    current.points.push({ x: round(x(i)), y: round(y(value)) });
  });

  return segments;
}

function toPath(segment: Segment) {
  return segment.points
    .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x},${p.y}`)
    .join(' ');
}

/**
 * A stateless inline-SVG history graph (FR-12).
 *
 * Hand-computed geometry modelled on `ContextUsageGauge` — no charting
 * library is added (research R7). Carries `role="img"` with a value-stating
 * `aria-label` so the graph has a text equivalent (FR-14).
 */
export function Sparkline({
  label,
  values,
  min = 0,
  max,
  width = 96,
  height = 24,
  strokeWidth = 1.5,
  valueText,
  severity,
  className,
}: SparklineProps) {
  const { t } = useTranslation('common');

  const readings = useMemo(
    () => values.filter((v): v is number => v !== null && Number.isFinite(v)),
    [values]
  );

  const domainMax = useMemo(() => {
    if (max !== undefined) return max;
    if (readings.length === 0) return min + 1;
    const largest = Math.max(...readings);
    return largest > min ? largest : min + 1;
  }, [max, min, readings]);

  const segments = useMemo(
    () =>
      buildSparklineSegments(values, {
        min,
        max: domainMax,
        width,
        height,
        strokeWidth,
      }),
    [values, min, domainMax, width, height, strokeWidth]
  );

  const latest = readings.length > 0 ? readings[readings.length - 1] : null;
  const span = domainMax - min;
  const ratio =
    latest === null || span <= 0 ? 0 : clamp((latest - min) / span, 0, 1);
  const bucket = severity ?? severityForRatio(ratio);

  const missingCount = values.length - readings.length;
  const latestText =
    latest === null
      ? t('metrics.noReading', { defaultValue: 'no reading' })
      : (valueText ?? String(round(latest)));

  const ariaLabel =
    missingCount > 0
      ? t('metrics.sparklineAriaWithGaps', {
          defaultValue:
            '{{label}} history: latest {{value}}, {{count}} readings, {{missing}} missing',
          label,
          value: latestText,
          count: values.length,
          missing: missingCount,
        })
      : t('metrics.sparklineAria', {
          defaultValue:
            '{{label}} history: latest {{value}}, {{count}} readings',
          label,
          value: latestText,
          count: values.length,
        });

  return (
    <div
      role="img"
      aria-label={ariaLabel}
      className={cn(
        'inline-flex items-center justify-center rounded-sm bg-panel',
        SEVERITY_TEXT_CLASS[bucket],
        className
      )}
      style={{ width, height }}
    >
      {readings.length === 0 ? (
        <span
          aria-hidden="true"
          data-testid="sparkline-no-reading"
          className="font-ibm-plex-mono text-sm text-low"
        >
          {NO_READING}
        </span>
      ) : (
        <svg
          viewBox={`0 0 ${width} ${height}`}
          width={width}
          height={height}
          aria-hidden="true"
          focusable="false"
        >
          {/* Segments are positional; a series has no stable ids. */}
          {segments.map((segment, i) =>
            segment.points.length === 1 ? (
              <circle
                key={i}
                data-testid="sparkline-point"
                cx={segment.points[0].x}
                cy={segment.points[0].y}
                r={strokeWidth}
                fill="currentColor"
              />
            ) : (
              <path
                key={i}
                data-testid="sparkline-segment"
                d={toPath(segment)}
                fill="none"
                stroke="currentColor"
                strokeWidth={strokeWidth}
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            )
          )}
        </svg>
      )}
    </div>
  );
}
