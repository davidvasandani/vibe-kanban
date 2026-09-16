import { useLocation, useNavigate } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { sessionsApi } from '@/shared/lib/api';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { workspaceSessionKeys } from '@/shared/hooks/workspaceSessionKeys';
import type { Session } from 'shared/types';

interface UseWorkspaceSessionsOptions {
  enabled?: boolean;
}

/** Discriminated union for session selection state */
export type SessionSelection =
  | { mode: 'existing'; sessionId: string }
  | { mode: 'new' };

interface UseWorkspaceSessionsResult {
  sessions: Session[];
  selectedSession: Session | undefined;
  selectedSessionId: string | undefined;
  selectSession: (sessionId: string) => void;
  selectLatestSession: () => void;
  isLoading: boolean;
  /** Whether user is creating a new session */
  isNewSessionMode: boolean;
  /** Enter new session mode */
  startNewSession: () => void;
}

/**
 * Hook for managing sessions within a workspace.
 * Fetches all sessions for a workspace and provides session switching capability.
 * Sessions are ordered by most recently used (latest non-dev server execution first).
 */
export function useWorkspaceSessions(
  workspaceId: string | undefined,
  options: UseWorkspaceSessionsOptions = {}
): UseWorkspaceSessionsResult {
  const hostId = useHostId();
  const location = useLocation();
  const navigate = useNavigate();
  const searchStr = location.searchStr;
  const requestedSessionId = new URLSearchParams(searchStr).get(
    'searchSessionId'
  );
  const appliedSearchRef = useRef<string>();
  const { enabled = true } = options;
  const [selection, setSelection] = useState<SessionSelection | undefined>(
    undefined
  );
  const prevWorkspaceIdRef = useRef(workspaceId);

  const { data: sessions = [], isLoading } = useQuery<Session[]>({
    queryKey: workspaceSessionKeys.byWorkspace(workspaceId, hostId),
    queryFn: () => sessionsApi.getByWorkspace(workspaceId!),
    enabled: enabled && !!workspaceId,
  });

  // Combined effect: handle workspace changes and auto-select sessions
  // This replaces two separate effects that had a race condition where the reset
  // effect would fire after auto-select when sessions were cached, undoing the selection.
  useEffect(() => {
    const workspaceChanged = prevWorkspaceIdRef.current !== workspaceId;
    prevWorkspaceIdRef.current = workspaceId;

    if (sessions.length > 0) {
      // Sessions are ordered by most recently used, so first is the most recently used
      // Preserve a valid explicit selection across refetches in the same workspace.
      setSelection((prev) => {
        if (
          !workspaceChanged &&
          (prev?.mode === 'new' ||
            (prev?.mode === 'existing' &&
              sessions.some((s) => s.id === prev.sessionId)))
        )
          return prev;
        return { mode: 'existing', sessionId: sessions[0].id };
      });
    } else {
      setSelection(undefined);
    }
  }, [workspaceId, sessions]);

  useEffect(() => {
    const requestKey = `${hostId}:${workspaceId}:${requestedSessionId}`;
    if (!requestedSessionId) {
      appliedSearchRef.current = undefined;
      return;
    }
    if (appliedSearchRef.current === requestKey) return;
    if (sessions.some((session) => session.id === requestedSessionId)) {
      appliedSearchRef.current = requestKey;
      setSelection({ mode: 'existing', sessionId: requestedSessionId });
    }
  }, [hostId, workspaceId, requestedSessionId, sessions]);

  const isNewSessionMode = selection?.mode === 'new' || sessions.length === 0;
  const selectedSessionId =
    selection?.mode === 'existing' ? selection.sessionId : undefined;

  const selectedSession = useMemo(
    () => sessions.find((s) => s.id === selectedSessionId),
    [sessions, selectedSessionId]
  );

  const clearSearchSession = useCallback(() => {
    if (!requestedSessionId) return;
    const params = new URLSearchParams(searchStr);
    params.delete('searchSessionId');
    const query = params.toString();
    void navigate({
      href: `${location.pathname}${query ? `?${query}` : ''}${location.hash ? `#${location.hash}` : ''}`,
      replace: true,
    });
  }, [
    requestedSessionId,
    searchStr,
    location.pathname,
    location.hash,
    navigate,
  ]);

  const selectSession = useCallback(
    (sessionId: string) => {
      clearSearchSession();
      setSelection({ mode: 'existing', sessionId });
    },
    [clearSearchSession]
  );

  const selectLatestSession = useCallback(() => {
    clearSearchSession();
    if (sessions.length > 0) {
      setSelection({ mode: 'existing', sessionId: sessions[0].id });
    }
  }, [sessions, clearSearchSession]);

  const startNewSession = useCallback(() => {
    clearSearchSession();
    setSelection({ mode: 'new' });
  }, [clearSearchSession]);

  return {
    sessions,
    selectedSession,
    selectedSessionId,
    selectSession,
    selectLatestSession,
    isLoading,
    isNewSessionMode,
    startNewSession,
  };
}
