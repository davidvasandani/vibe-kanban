import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { McpRefreshResult } from 'shared/types';
import { toast } from 'sonner';
import { sessionsApi } from '@/shared/lib/api';

interface McpRefreshApi {
  refreshMcpTools: (
    workspaceId: string,
    sessionId: string
  ) => Promise<McpRefreshResult>;
  getMcpRefreshStatus: (
    workspaceId: string,
    sessionId: string
  ) => Promise<McpRefreshResult | null>;
}

interface UseMcpRefreshOptions {
  api?: McpRefreshApi;
  pollIntervalMs?: number;
}

function notifyResult(result: McpRefreshResult) {
  if (result.status === 'pending_next_turn') {
    toast.info('MCP refresh queued for the next agent turn.');
  } else if (result.status === 'unsupported') {
    toast.info(
      result.error?.message ??
        'This executor cannot refresh MCP tools in place.'
    );
  } else if (result.status === 'busy') {
    toast.info(
      result.error?.message ?? 'An MCP refresh is already in progress.'
    );
  } else if (result.status === 'refreshed') {
    toast.success('MCP tools refreshed.');
  } else if (result.status === 'partially_refreshed') {
    toast.warning('MCP tools refreshed with one or more server failures.');
  } else {
    toast.error(result.error?.message ?? 'MCP refresh failed.');
  }
}

export function useMcpRefresh(
  workspaceId: string | undefined,
  sessionId: string | undefined,
  options: UseMcpRefreshOptions = {}
) {
  const api = options.api ?? sessionsApi;
  const pollIntervalMs = options.pollIntervalMs ?? 2000;
  const sessionKey = workspaceId && sessionId ? `${workspaceId}:${sessionId}` : null;
  const activeSessionKeyRef = useRef(sessionKey);
  activeSessionKeyRef.current = sessionKey;

  const [isRefreshing, setIsRefreshing] = useState(false);
  const [result, setResult] = useState<McpRefreshResult | null>(null);
  const notifiedResultRef = useRef<string | null>(null);

  const applyStatus = useCallback(
    (
      expectedSessionKey: string,
      nextResult: McpRefreshResult | null,
      notify: boolean
    ) => {
      if (activeSessionKeyRef.current !== expectedSessionKey) return false;
      setResult(nextResult);
      if (notify && nextResult) {
        const notificationKey = `${nextResult.generation}:${nextResult.status}`;
        if (notifiedResultRef.current !== notificationKey) {
          notifiedResultRef.current = notificationKey;
          notifyResult(nextResult);
        }
      }
      return true;
    },
    []
  );

  const readCanonicalStatus = useCallback(
    async (expectedSessionKey: string, notify: boolean) => {
      if (!workspaceId || !sessionId) return null;
      const nextResult = await api.getMcpRefreshStatus(workspaceId, sessionId);
      applyStatus(expectedSessionKey, nextResult, notify);
      return nextResult;
    },
    [api, applyStatus, sessionId, workspaceId]
  );

  useEffect(() => {
    setResult(null);
    setIsRefreshing(false);
    notifiedResultRef.current = null;
    if (!sessionKey) return;

    void readCanonicalStatus(sessionKey, false).catch(() => {
      // An initial read is advisory. A later click or poll can recover.
    });
  }, [readCanonicalStatus, sessionKey]);

  useEffect(() => {
    if (!sessionKey || result?.status !== 'pending_next_turn') return;
    const timer = window.setInterval(() => {
      void readCanonicalStatus(sessionKey, true).catch(() => {
        // Keep the last confirmed state; a later poll may recover.
      });
    }, pollIntervalMs);
    return () => window.clearInterval(timer);
  }, [pollIntervalMs, readCanonicalStatus, result?.status, sessionKey]);

  const refresh = useCallback(async () => {
    if (!workspaceId || !sessionId || !sessionKey || isRefreshing) return;
    setIsRefreshing(true);
    try {
      const nextResult = await api.refreshMcpTools(workspaceId, sessionId);
      if (!applyStatus(sessionKey, nextResult, true)) return;
      if (nextResult.status === 'busy') {
        await readCanonicalStatus(sessionKey, false);
      }
    } catch {
      if (activeSessionKeyRef.current === sessionKey) {
        toast.error('MCP refresh failed.');
      }
    } finally {
      if (activeSessionKeyRef.current === sessionKey) {
        setIsRefreshing(false);
      }
    }
  }, [
    api,
    applyStatus,
    isRefreshing,
    readCanonicalStatus,
    sessionId,
    sessionKey,
    workspaceId,
  ]);

  const tooltip = useMemo(() => {
    if (!result) {
      return 'Reload MCP configuration for this session without replacing its conversation';
    }
    const refreshedAt = result.last_successful_refresh_at
      ? new Date(result.last_successful_refresh_at).toLocaleString()
      : 'not yet confirmed';
    const servers = result.servers
      .map(
        (server) =>
          `${server.server_id}: ${server.status}, ${
            server.tool_count == null
              ? 'tool count unknown'
              : `${server.tool_count} tools`
          }, ${
            server.restart_occurred == null
              ? 'restart unknown'
              : server.restart_occurred
                ? 'restarted'
                : 'reused'
          }`
      )
      .join('; ');
    return `MCP refresh: ${result.status}. Last successful: ${refreshedAt}${
      servers ? `. ${servers}` : ''
    }`;
  }, [result]);

  return {
    isRefreshing,
    refresh,
    result,
    tooltip,
  };
}
