import type { McpServerTestResult } from 'shared/types';

export type McpToolCountSummary = {
  minimum: number;
  maximum: number;
};

export function summarizeMcpToolCounts(
  results: Array<McpServerTestResult | undefined>
): McpToolCountSummary | null {
  const counts = results.flatMap((result) =>
    result?.status === 'ok' && result.tool_count !== null
      ? [result.tool_count]
      : []
  );
  if (counts.length === 0) return null;

  return {
    minimum: Math.min(...counts),
    maximum: Math.max(...counts),
  };
}

export function formatMcpCheckedAt(
  checkedAt: number,
  locale: string,
  options: Intl.DateTimeFormatOptions = {}
): string {
  return new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'short',
    ...options,
  }).format(checkedAt);
}
