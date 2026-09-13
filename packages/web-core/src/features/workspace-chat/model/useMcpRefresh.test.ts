import { describe, expect, it } from 'vitest';
import type { McpRefreshResult } from 'shared/types';
import { mcpRefreshTooltip } from './useMcpRefresh';

function result(overrides: Partial<McpRefreshResult> = {}): McpRefreshResult {
  return {
    status: 'refreshed',
    retryable: false,
    generation: 1n,
    requested_at: '2026-09-10T00:00:00Z',
    last_successful_refresh_at: null,
    configured_server_ids: ['entra', 'slack'],
    servers: [],
    error: null,
    ...overrides,
  };
}

describe('mcpRefreshTooltip', () => {
  it('calls out a configured Entra server absent from the active registry', () => {
    const tooltip = mcpRefreshTooltip(result());

    expect(tooltip).toContain(
      'entra is configured but absent from the active executor tool registry'
    );
    expect(tooltip).toContain(
      'Restart the session or open a diagnostic issue'
    );
  });

  it('calls out a registered Entra server with no usable tools', () => {
    const tooltip = mcpRefreshTooltip(
      result({
        servers: [
          {
            server_id: 'entra',
            status: 'ready',
            tool_count: 0,
            resource_count: 0,
            prompt_count: 0,
            restart_occurred: null,
            error: null,
          },
        ],
      })
    );

    expect(tooltip).toContain(
      'entra connected, but 0 tools are registered in the active executor'
    );
  });

  it('reports the ready three-tool Entra capability', () => {
    const tooltip = mcpRefreshTooltip(
      result({
        servers: [
          {
            server_id: 'entra',
            status: 'ready',
            tool_count: 3,
            resource_count: 0,
            prompt_count: 0,
            restart_occurred: null,
            error: null,
          },
        ],
      })
    );

    expect(tooltip).toContain('entra is available with 3 tools');
  });
});
