import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { McpRefreshResult } from 'shared/types';
import { toast } from 'sonner';
import { sessionsApi } from '@/shared/lib/api';
import { mcpCapabilityDiagnostic } from './mcpCapabilityDiagnostic';

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
  } else if (result.status === 'refreshed') {
    toast.success('MCP tools refreshed.');
  } else if (result.status === 'partially_refreshed') {
    toast.warning('MCP tools refreshed with one or more server failures.');
  } else if (result.status === 'busy' || result.status === 'unsupported') {
    toast.info(result.error?.message ?? 'MCP refresh is unavailable.');
  } else {
    toast.error(result.error?.message ?? 'MCP refresh failed.');
  }
}

export function mcpRefreshTooltip(result: McpRefreshResult | null) {
  if (!result) {
    return 'Reload MCP configuration and verify the active executor tool registry';
  }
  const slack = mcpCapabilityDiagnostic(result, 'slack');
  const entra = mcpCapabilityDiagnostic(result, 'entra');
  return `MCP refresh: ${result.status}. ${slack.message} ${entra.message}`;
}

export function useMcpRefresh(
  workspaceId: string | undefined,
  sessionId: string | undefined,
  options: UseMcpRefreshOptions = {}
) {
  const api = options.api ?? sessionsApi;
  const pollIntervalMs = options.pollIntervalMs ?? 2000;
  const sessionKey =
    workspaceId && sessionId ? `${workspaceId}:${sessionId}` : null;
  const activeSessionKey = useRef(sessionKey);
  activeSessionKey.current = sessionKey;
  const [result, setResult] = useState<McpRefreshResult | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isReconcilingBusy, setIsReconcilingBusy] = useState(false);
  const operation = useRef(0);
  const activeRefreshOperation = useRef<{
    sessionKey: string;
    operation: number;
  } | null>(null);

  const applyResult = useCallback(
    (
      expectedSessionKey: string,
      expectedOperation: number,
      next: McpRefreshResult | null
    ) => {
      if (
        activeSessionKey.current !== expectedSessionKey ||
        operation.current !== expectedOperation
      ) {
        return false;
      }
      setResult(next);
      setIsReconcilingBusy(false);
      return true;
    },
    []
  );

  const readStatus = useCallback(async () => {
    if (!workspaceId || !sessionId || !sessionKey) return null;
    const currentOperation = ++operation.current;
    const next = await api.getMcpRefreshStatus(workspaceId, sessionId);
    applyResult(sessionKey, currentOperation, next);
    return next;
  }, [api, applyResult, sessionId, sessionKey, workspaceId]);

  useEffect(() => {
    setResult(null);
    setIsRefreshing(false);
    setIsReconcilingBusy(false);
    activeRefreshOperation.current = null;
    if (sessionKey) void readStatus().catch(() => undefined);
  }, [readStatus, sessionKey]);

  useEffect(() => {
    if (
      !sessionKey ||
      (result?.status !== 'pending_next_turn' && !isReconcilingBusy)
    )
      return;
    let cancelled = false;
    let timer: number;
    const poll = async () => {
      const next = await readStatus().catch(() => result);
      if (
        !cancelled &&
        (next?.status === 'pending_next_turn' || isReconcilingBusy)
      ) {
        timer = window.setTimeout(poll, pollIntervalMs);
      }
    };
    timer = window.setTimeout(poll, pollIntervalMs);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [isReconcilingBusy, pollIntervalMs, readStatus, result, sessionKey]);

  const refresh = useCallback(async () => {
    if (!workspaceId || !sessionId || !sessionKey || isRefreshing) return;
    setIsRefreshing(true);
    const currentOperation = ++operation.current;
    const refreshOperation = { sessionKey, operation: currentOperation };
    activeRefreshOperation.current = refreshOperation;
    try {
      const next = await api.refreshMcpTools(workspaceId, sessionId);
      if (next.status === 'busy') {
        setIsReconcilingBusy(true);
        await readStatus().catch(() => undefined);
      } else {
        applyResult(sessionKey, currentOperation, next);
      }
      if (activeSessionKey.current === sessionKey) notifyResult(next);
    } catch {
      toast.error('MCP refresh failed.');
    } finally {
      if (activeRefreshOperation.current === refreshOperation) {
        activeRefreshOperation.current = null;
        setIsRefreshing(false);
      }
    }
  }, [
    api,
    applyResult,
    isRefreshing,
    readStatus,
    sessionId,
    sessionKey,
    workspaceId,
  ]);

  const tooltip = useMemo(() => {
    return mcpRefreshTooltip(result);
  }, [result]);

  return {
    isRefreshing: isRefreshing || isReconcilingBusy,
    refresh,
    result,
    tooltip,
  };
}
