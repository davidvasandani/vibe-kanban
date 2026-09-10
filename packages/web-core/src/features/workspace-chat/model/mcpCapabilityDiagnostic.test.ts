import { describe, expect, it } from 'vitest';
import type {
  McpRefreshResult,
  McpRefreshStatus,
  McpServerRefreshSnapshot,
} from 'shared/types';
import { mcpCapabilityDiagnostic } from './mcpCapabilityDiagnostic';

function result(
  status: McpRefreshStatus,
  servers: McpServerRefreshSnapshot[] = [],
  configured = ['slack']
): McpRefreshResult {
  return {
    status,
    retryable: false,
    generation: 1n,
    requested_at: '2026-09-03T00:00:00Z',
    last_successful_refresh_at: null,
    configured_server_ids: configured,
    servers,
    error: null,
  };
}

const ready: McpServerRefreshSnapshot = {
  server_id: 'slack',
  status: 'ready',
  tool_count: 1,
  resource_count: 0,
  prompt_count: 0,
  restart_occurred: null,
  error: null,
};

describe('mcpCapabilityDiagnostic', () => {
  it('does not confuse an enabled setting with agent-visible availability', () => {
    expect(mcpCapabilityDiagnostic(result('refreshed'), 'slack')).toMatchObject(
      {
        state: 'unavailable',
      }
    );
  });

  it('keeps pending adoption distinct from availability', () => {
    expect(
      mcpCapabilityDiagnostic(result('pending_next_turn'), 'slack').state
    ).toBe('needs-refresh');
  });

  it('rejects a registered server with zero tools', () => {
    expect(
      mcpCapabilityDiagnostic(
        result('refreshed', [{ ...ready, tool_count: 0 }]),
        'slack'
      ).state
    ).toBe('unavailable');
  });

  it('reports ready only with a ready server and a positive tool count', () => {
    expect(
      mcpCapabilityDiagnostic(result('refreshed', [ready]), 'slack')
    ).toEqual({
      state: 'available',
      message: 'slack is available with 1 tool.',
    });
  });

  it('does not report retained snapshots as available after refresh failure', () => {
    const failed = result('failed', [ready]);
    failed.error = {
      category: 'reload_failed',
      message: 'MCP reload failed.',
      remediation: 'Retry the refresh.',
      retryable: true,
    };
    expect(mcpCapabilityDiagnostic(failed, 'slack')).toEqual({
      state: 'unavailable',
      message: 'MCP reload failed.',
    });
  });

  it('distinguishes a server that is not assigned', () => {
    expect(
      mcpCapabilityDiagnostic(result('refreshed', [], []), 'slack').state
    ).toBe('not-configured');
  });
});
