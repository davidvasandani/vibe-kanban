import { useEffect, useState } from 'react';
import { useIsMobile } from '@/shared/hooks/useIsMobile';
import { useMobileActiveTab } from '@/shared/stores/useUiPreferencesStore';
import { useIsChatHistorySettled } from '@/shared/stores/useChatHistoryReadyStore';

/** Mobile tabs that render the workspace diff. */
const DIFF_TABS: ReadonlySet<string> = new Set(['changes', 'git']);

export interface DiffStreamGateState {
  isMobile: boolean;
  mobileTab: string;
  chatHistorySettled: boolean;
}

/**
 * Whether the workspace diff stream is wanted now. On a phone, each websocket
 * is a queued handshake, and this one is large and slow to its first frame
 * (613 KB, 15–42 s cold over NFS). It therefore waits until a diff tab is
 * shown or the chat has rendered its history; the chat's diff-stats pill
 * fills in after the conversation. Desktop opens it immediately, as before.
 */
export function wantsWorkspaceDiffStream({
  isMobile,
  mobileTab,
  chatHistorySettled,
}: DiffStreamGateState): boolean {
  return !isMobile || DIFF_TABS.has(mobileTab) || chatHistorySettled;
}

/**
 * The diff stream's `enabled` flag. Once it has been wanted for a workspace it
 * stays enabled for that workspace, so switching tabs never closes it.
 */
export function useWorkspaceDiffStreamGate(
  workspaceId: string | undefined,
  isCreateMode: boolean
): boolean {
  const isMobile = useIsMobile();
  const [mobileTab] = useMobileActiveTab();
  const chatHistorySettled = useIsChatHistorySettled(workspaceId);
  const wanted = wantsWorkspaceDiffStream({
    isMobile,
    mobileTab,
    chatHistorySettled,
  });
  const [enabledFor, setEnabledFor] = useState<string | null>(null);

  useEffect(() => {
    if (wanted && workspaceId) setEnabledFor(workspaceId);
  }, [wanted, workspaceId]);

  if (isCreateMode || !workspaceId) return false;
  return wanted || enabledFor === workspaceId;
}
