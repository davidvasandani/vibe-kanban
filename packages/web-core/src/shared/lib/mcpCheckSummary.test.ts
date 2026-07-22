import { describe, expect, it } from 'vitest';
import type { McpServerTestResult } from 'shared/types';
import { formatMcpCheckedAt, summarizeMcpToolCounts } from './mcpCheckSummary';

function result(
  status: McpServerTestResult['status'],
  toolCount: number | null
): McpServerTestResult {
  return {
    name: 'server',
    transport: 'http',
    status,
    latency_ms: null,
    tool_count: toolCount,
    server_name: null,
    server_version: null,
    error: null,
    www_authenticate: null,
  };
}

describe('MCP check summary', () => {
  it('returns no count without a successful known tool count', () => {
    expect(summarizeMcpToolCounts([])).toBeNull();
    expect(
      summarizeMcpToolCounts([
        undefined,
        result('failed', 10),
        result('auth_required', null),
        result('ok', null),
      ])
    ).toBeNull();
  });

  it('returns one count for one result or identical assignment counts', () => {
    expect(summarizeMcpToolCounts([result('ok', 1)])).toEqual({
      minimum: 1,
      maximum: 1,
    });
    expect(
      summarizeMcpToolCounts([result('ok', 36), result('ok', 36)])
    ).toEqual({ minimum: 36, maximum: 36 });
  });

  it('returns the inclusive range for differing assignment counts', () => {
    expect(
      summarizeMcpToolCounts([
        result('ok', 36),
        result('failed', 100),
        result('ok', 34),
      ])
    ).toEqual({ minimum: 34, maximum: 36 });
  });

  it('formats the checked timestamp using the requested locale', () => {
    const timestamp = Date.UTC(2026, 6, 22, 2, 59);
    expect(formatMcpCheckedAt(timestamp, 'en-US', { timeZone: 'UTC' })).toBe(
      'Jul 22, 2026, 2:59 AM'
    );
    expect(formatMcpCheckedAt(timestamp, 'en-GB', { timeZone: 'UTC' })).toBe(
      '22 Jul 2026, 02:59'
    );
  });
});
