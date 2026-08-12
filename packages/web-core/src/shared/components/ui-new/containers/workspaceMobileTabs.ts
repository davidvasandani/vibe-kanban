import {
  MOBILE_TABS,
  type MobileTabDefinition,
  type MobileTabId,
} from '@vibe/ui/components/Navbar';

interface WorkspaceMobileTabState {
  hasWorkspaceRoute: boolean;
  isCreateMode: boolean;
}

const hasUsableRightSidebar = ({
  hasWorkspaceRoute,
  isCreateMode,
}: WorkspaceMobileTabState) => hasWorkspaceRoute && !isCreateMode;

export function getAvailableWorkspaceMobileTabs(
  state: WorkspaceMobileTabState
): MobileTabDefinition[] {
  return hasUsableRightSidebar(state)
    ? MOBILE_TABS
    : MOBILE_TABS.filter((tab) => tab.id !== 'git');
}

export function getWorkspaceMobileTabFallback(
  currentTab: MobileTabId,
  state: WorkspaceMobileTabState
): MobileTabId {
  if (currentTab !== 'git' || hasUsableRightSidebar(state)) {
    return currentTab;
  }

  return state.isCreateMode ? 'chat' : 'workspaces';
}
