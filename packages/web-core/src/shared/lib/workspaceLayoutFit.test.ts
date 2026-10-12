import { describe, expect, it } from 'vitest';
import {
  LEFT_SIDEBAR_WIDTH,
  MIN_CHAT_WIDTH,
  MIN_RIGHT_MAIN_PANEL_WIDTH,
  RIGHT_SIDEBAR_WIDTH,
  leftSidebarFits,
  requiredWidthForLeftSidebar,
} from './workspaceLayoutFit';

const chatAndRightSidebar = {
  isChatShown: true,
  isRightMainPanelOpen: false,
  isRightSidebarShown: true,
};
const withChanges = { ...chatAndRightSidebar, isRightMainPanelOpen: true };

describe('requiredWidthForLeftSidebar', () => {
  it('counts only the panels that are on screen', () => {
    expect(requiredWidthForLeftSidebar(chatAndRightSidebar)).toBe(
      LEFT_SIDEBAR_WIDTH + RIGHT_SIDEBAR_WIDTH + MIN_CHAT_WIDTH
    );
    expect(requiredWidthForLeftSidebar(withChanges)).toBe(
      LEFT_SIDEBAR_WIDTH +
        RIGHT_SIDEBAR_WIDTH +
        MIN_CHAT_WIDTH +
        MIN_RIGHT_MAIN_PANEL_WIDTH
    );
    expect(
      requiredWidthForLeftSidebar({
        isChatShown: false,
        isRightMainPanelOpen: true,
        isRightSidebarShown: false,
      })
    ).toBe(LEFT_SIDEBAR_WIDTH + MIN_RIGHT_MAIN_PANEL_WIDTH);
  });
});

describe('leftSidebarFits', () => {
  it('shows the drawer beside the chat in a mid-sized window', () => {
    // ~1300px window minus the ~130px app rail.
    expect(leftSidebarFits({ width: 1170, ...chatAndRightSidebar })).toBe(true);
  });

  it('keeps the drawer with Changes open on a 1920px screen', () => {
    expect(leftSidebarFits({ width: 1790, ...withChanges })).toBe(true);
  });

  it('hides the drawer with Changes and the right sidebar on a laptop', () => {
    expect(leftSidebarFits({ width: 1310, ...withChanges })).toBe(false);
    expect(
      leftSidebarFits({
        width: 1310,
        ...withChanges,
        isRightSidebarShown: false,
      })
    ).toBe(true);
  });

  it('is inclusive at the exact required width', () => {
    const required = requiredWidthForLeftSidebar(chatAndRightSidebar);
    expect(leftSidebarFits({ width: required, ...chatAndRightSidebar })).toBe(
      true
    );
    expect(
      leftSidebarFits({ width: required - 1, ...chatAndRightSidebar })
    ).toBe(false);
  });
});
