import { NO_READING } from '@vibe/ui/components/Meter';

export { NO_READING };

/**
 * Widens a generated numeric field to a plain `number`.
 *
 * `ts-rs` maps Rust's `u64` to `bigint`, but `JSON.parse` produces a `number`
 * for the same wire value, so both arrive at runtime. `null`/`undefined` — the
 * "no reading was taken" case — survives as `null` and must never become `0`.
 */
export function toNumber(
  value: bigint | number | null | undefined
): number | null {
  if (value === null || value === undefined) return null;
  const asNumber = typeof value === 'bigint' ? Number(value) : value;
  return Number.isFinite(asNumber) ? asNumber : null;
}

const BYTE_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const;

/** Binary byte size, e.g. `1.2 GiB`. `null` renders as an em dash. */
export function formatBytes(value: bigint | number | null | undefined): string {
  const bytes = toNumber(value);
  if (bytes === null) return NO_READING;
  const sign = bytes < 0 ? '-' : '';
  let magnitude = Math.abs(bytes);
  let unit = 0;
  while (magnitude >= 1024 && unit < BYTE_UNITS.length - 1) {
    magnitude /= 1024;
    unit += 1;
  }
  const digits =
    unit === 0 ? 0 : magnitude >= 100 ? 0 : magnitude >= 10 ? 1 : 2;
  return `${sign}${magnitude.toFixed(digits)} ${BYTE_UNITS[unit]}`;
}

/** Byte rate, e.g. `4.0 MiB/s`. */
export function formatBytesPerSecond(
  value: bigint | number | null | undefined
): string {
  const rate = toNumber(value);
  if (rate === null) return NO_READING;
  return `${formatBytes(rate)}/s`;
}

/** Percentage with one decimal, e.g. `42.3%`. */
export function formatPercent(
  value: bigint | number | null | undefined,
  digits = 1
): string {
  const percent = toNumber(value);
  if (percent === null) return NO_READING;
  return `${percent.toFixed(digits)}%`;
}

/** A plain count, e.g. a thread count or a core count. */
export function formatCount(value: bigint | number | null | undefined): string {
  const count = toNumber(value);
  if (count === null) return NO_READING;
  return String(count);
}

/** Load average, which is not a percentage and has its own precision. */
export function formatLoad(value: number | null | undefined): string {
  const load = toNumber(value);
  if (load === null) return NO_READING;
  return load.toFixed(2);
}

/** `used / total` as a 0..100 percentage, or `null` if either side is absent. */
export function ratioPercent(
  used: bigint | number | null | undefined,
  total: bigint | number | null | undefined
): number | null {
  const usedNumber = toNumber(used);
  const totalNumber = toNumber(total);
  if (usedNumber === null || totalNumber === null || totalNumber <= 0) {
    return null;
  }
  return (usedNumber / totalNumber) * 100;
}

/**
 * A wall-clock timestamp for a retained reading.
 *
 * Stale readings are labelled with *when they were taken* rather than being
 * presented as current (FR-18), so this is deliberately absolute rather than
 * a relative "2m ago".
 */
export function formatTimestamp(value: string | null | undefined): string {
  if (!value) return NO_READING;
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return NO_READING;
  return parsed.toLocaleTimeString();
}

/** Elapsed seconds as `3d 4h 5m`. */
export function formatUptime(
  value: bigint | number | null | undefined
): string {
  const seconds = toNumber(value);
  if (seconds === null) return NO_READING;
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}
