import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.stubGlobal('window', {
  matchMedia: () => ({ matches: false }),
});

const {
  RIGHT_MAIN_PANEL_MODES,
  selectIsLeftSidebarShown,
  useUiPreferencesStore,
} = await import('./useUiPreferencesStore');

const WS = 'workspace-1';
const store = () => useUiPreferencesStore.getState();
const shown = () => selectIsLeftSidebarShown(store());

beforeEach(() => {
  useUiPreferencesStore.setState({
    isLeftSidebarVisible: true,
    leftSidebarHasSpace: true,
    isLeftSidebarForcedOpen: false,
    workspacePanelStates: {},
  });
});

describe('left drawer and right main panels', () => {
  it('opening a right panel keeps the saved preference', () => {
    store().toggleRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.CHANGES, WS);
    expect(store().isLeftSidebarVisible).toBe(true);

    store().setRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.LOGS, WS);
    expect(store().isLeftSidebarVisible).toBe(true);
  });

  it('closing a right panel with its toggle restores the drawer', () => {
    store().toggleRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.CHANGES, WS);
    store().setLeftSidebarVisible(false);
    store().toggleRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.CHANGES, WS);
    expect(store().isLeftSidebarVisible).toBe(true);
  });
});

describe('left drawer space', () => {
  it('hides for lack of space without touching the preference', () => {
    store().setLeftSidebarHasSpace(false);
    expect(shown()).toBe(false);
    expect(store().isLeftSidebarVisible).toBe(true);

    store().setLeftSidebarHasSpace(true);
    expect(shown()).toBe(true);
  });
});

describe('toggleLeftSidebar', () => {
  it('hides a shown drawer and saves that', () => {
    store().toggleLeftSidebar();
    expect(shown()).toBe(false);
    expect(store().isLeftSidebarVisible).toBe(false);
  });

  it('shows a drawer the user hid', () => {
    store().setLeftSidebarVisible(false);
    store().toggleLeftSidebar();
    expect(shown()).toBe(true);
    expect(store().isLeftSidebarVisible).toBe(true);
    expect(store().isLeftSidebarForcedOpen).toBe(false);
  });

  it('shows a drawer hidden for lack of space until space changes', () => {
    store().setLeftSidebarHasSpace(false);
    store().toggleLeftSidebar();
    expect(shown()).toBe(true);
    expect(store().isLeftSidebarForcedOpen).toBe(true);

    // Same space reading again: the override holds.
    store().setLeftSidebarHasSpace(false);
    expect(shown()).toBe(true);

    // Space comes back, then goes again: the drawer follows the space.
    store().setLeftSidebarHasSpace(true);
    expect(store().isLeftSidebarForcedOpen).toBe(false);
    store().setLeftSidebarHasSpace(false);
    expect(shown()).toBe(false);
  });

  it('shows in one toggle when hidden by preference and lacking space', () => {
    store().setLeftSidebarVisible(false);
    store().setLeftSidebarHasSpace(false);
    store().toggleLeftSidebar();
    expect(shown()).toBe(true);

    store().toggleLeftSidebar();
    expect(shown()).toBe(false);
    expect(store().isLeftSidebarVisible).toBe(false);
    expect(store().isLeftSidebarForcedOpen).toBe(false);
  });
});
