import { useCallback } from 'react';
import { create } from 'zustand';

type State = {
  // Selected browser session id per workspace
  selectedSessionIds: Record<string, string | null>;
  // Bumped when the session list should be refetched (e.g. WS state changes)
  listRefreshNonce: number;

  setSelectedSessionId: (workspaceId: string, sessionId: string | null) => void;
  bumpListRefresh: () => void;
};

export const useBrowserSessionStore = create<State>()((set) => ({
  selectedSessionIds: {},
  listRefreshNonce: 0,

  setSelectedSessionId: (workspaceId, sessionId) =>
    set((s) => ({
      selectedSessionIds: {
        ...s.selectedSessionIds,
        [workspaceId]: sessionId,
      },
    })),

  bumpListRefresh: () =>
    set((s) => ({ listRefreshNonce: s.listRefreshNonce + 1 })),
}));

// Hook for the selected browser session of a workspace
export function useSelectedBrowserSessionId(
  workspaceId: string | undefined
): [string | null, (sessionId: string | null) => void] {
  const sessionId = useBrowserSessionStore((s) =>
    workspaceId ? (s.selectedSessionIds[workspaceId] ?? null) : null
  );
  const setSelectedSessionId = useBrowserSessionStore(
    (s) => s.setSelectedSessionId
  );

  const setForWorkspace = useCallback(
    (id: string | null) => {
      if (workspaceId) setSelectedSessionId(workspaceId, id);
    },
    [workspaceId, setSelectedSessionId]
  );

  return [sessionId, setForWorkspace];
}
