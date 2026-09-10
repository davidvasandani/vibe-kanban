import type { McpRefreshResult } from 'shared/types';

export type McpCapabilityState =
  | 'not-configured'
  | 'needs-refresh'
  | 'unavailable'
  | 'available';

export interface McpCapabilityDiagnostic {
  state: McpCapabilityState;
  message: string;
}

export function mcpCapabilityDiagnostic(
  result: McpRefreshResult | null,
  serverId: string
): McpCapabilityDiagnostic {
  if (!result?.configured_server_ids.includes(serverId)) {
    return {
      state: 'not-configured',
      message: `${serverId} is not assigned to this Codex profile.`,
    };
  }
  if (result.status === 'pending_next_turn' || result.status === 'busy') {
    return {
      state: 'needs-refresh',
      message: `${serverId} is configured; adoption is waiting for the next Codex turn.`,
    };
  }
  if (result.status === 'failed') {
    return {
      state: 'unavailable',
      message:
        result.error?.message ??
        `${serverId} availability could not be confirmed after the MCP refresh failed.`,
    };
  }
  const server = result.servers.find((item) => item.server_id === serverId);
  if (!server) {
    return {
      state: 'unavailable',
      message: `${serverId} is configured but absent from the active Codex tool registry. Refresh MCP tools or restart the agent.`,
    };
  }
  if (server.status !== 'ready' || (server.tool_count ?? 0) === 0) {
    return {
      state: 'unavailable',
      message:
        server.error?.message ??
        `${serverId} is registered but exposes no usable tools. Reconnect it, then refresh MCP tools.`,
    };
  }
  return {
    state: 'available',
    message: `${serverId} is available with ${server.tool_count} tool${server.tool_count === 1 ? '' : 's'}.`,
  };
}
