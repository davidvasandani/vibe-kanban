// Widths the desktop workspace layout needs before the left drawer (the
// workspaces list) fits beside the content that is open. The sidebars are
// fixed-width columns in WorkspacesLayout; the minimums keep the chat composer
// and a right main panel usable.
export const LEFT_SIDEBAR_WIDTH = 300;
export const RIGHT_SIDEBAR_WIDTH = 300;
export const MIN_CHAT_WIDTH = 480;
export const MIN_RIGHT_MAIN_PANEL_WIDTH = 400;

export interface LeftSidebarFitInput {
  /** Measured width of the workspace layout root, in CSS px. */
  width: number;
  isChatShown: boolean;
  isRightMainPanelOpen: boolean;
  isRightSidebarShown: boolean;
}

export function requiredWidthForLeftSidebar({
  isChatShown,
  isRightMainPanelOpen,
  isRightSidebarShown,
}: Omit<LeftSidebarFitInput, 'width'>): number {
  return (
    LEFT_SIDEBAR_WIDTH +
    (isRightSidebarShown ? RIGHT_SIDEBAR_WIDTH : 0) +
    (isChatShown ? MIN_CHAT_WIDTH : 0) +
    (isRightMainPanelOpen ? MIN_RIGHT_MAIN_PANEL_WIDTH : 0)
  );
}

export function leftSidebarFits({ width, ...rest }: LeftSidebarFitInput) {
  return width >= requiredWidthForLeftSidebar(rest);
}
