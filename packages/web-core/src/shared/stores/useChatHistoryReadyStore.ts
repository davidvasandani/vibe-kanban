import { create } from 'zustand';

type State = {
  /** Workspaces whose chat has shown its first settled history this page. */
  settledWorkspaceIds: Record<string, true>;
  markSettled: (workspaceId: string) => void;
};

/**
 * Lets panels that are not on screen wait for the chat. On the phone each
 * websocket is a queued handshake, so a panel that would open one (the
 * workspace diff stream, for example) defers it until the chat's first
 * history view has rendered.
 */
export const useChatHistoryReadyStore = create<State>()((set) => ({
  settledWorkspaceIds: {},
  markSettled: (workspaceId) =>
    set((state) =>
      state.settledWorkspaceIds[workspaceId]
        ? state
        : {
            settledWorkspaceIds: {
              ...state.settledWorkspaceIds,
              [workspaceId]: true,
            },
          }
    ),
}));

export function useIsChatHistorySettled(
  workspaceId: string | null | undefined
): boolean {
  return useChatHistoryReadyStore((state) =>
    workspaceId ? state.settledWorkspaceIds[workspaceId] === true : false
  );
}
