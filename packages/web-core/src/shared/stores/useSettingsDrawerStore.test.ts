import { beforeEach, describe, expect, it, vi } from 'vitest';

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
});

const {
  SETTINGS_DRAWER_APP_RESERVE,
  SETTINGS_DRAWER_DEFAULT_WIDTH,
  SETTINGS_DRAWER_MAX_WIDTH,
  SETTINGS_DRAWER_MIN_WIDTH,
  canDockSettingsDrawer,
  clampSettingsDrawerWidth,
  toggleSettingsDrawerWith,
  useSettingsDrawerStore,
} = await import('./useSettingsDrawerStore');

describe('clampSettingsDrawerWidth', () => {
  it('keeps a width inside the bounds unchanged', () => {
    expect(clampSettingsDrawerWidth(700, 1600)).toBe(700);
  });

  it('never goes below the minimum', () => {
    expect(clampSettingsDrawerWidth(100, 1600)).toBe(SETTINGS_DRAWER_MIN_WIDTH);
  });

  it('caps at the absolute maximum on wide screens', () => {
    expect(clampSettingsDrawerWidth(5000, 4000)).toBe(
      SETTINGS_DRAWER_MAX_WIDTH
    );
  });

  it('always leaves the app reserve beside the drawer', () => {
    expect(clampSettingsDrawerWidth(1100, 1300)).toBe(
      1300 - SETTINGS_DRAWER_APP_RESERVE
    );
  });

  it('prefers the minimum when the viewport cannot fit the reserve', () => {
    expect(clampSettingsDrawerWidth(800, 800)).toBe(SETTINGS_DRAWER_MIN_WIDTH);
  });
});

describe('useSettingsDrawerStore', () => {
  beforeEach(() => {
    useSettingsDrawerStore.setState({
      isOpen: false,
      width: SETTINGS_DRAWER_DEFAULT_WIDTH,
      requestClose: null,
    });
  });

  it('starts closed at the default width', () => {
    const state = useSettingsDrawerStore.getState();
    expect(state.isOpen).toBe(false);
    expect(state.width).toBe(SETTINGS_DRAWER_DEFAULT_WIDTH);
  });

  it('persists only the width', () => {
    useSettingsDrawerStore.getState().setOpen(true);
    useSettingsDrawerStore.getState().setWidth(812.4);
    const persisted = JSON.parse(
      storage.get('vibe.ui.settingsDrawerWidth') ?? '{}'
    );
    expect(persisted.state).toEqual({ width: 812 });
  });

  it('does not store a width below the minimum', () => {
    useSettingsDrawerStore.getState().setWidth(10);
    expect(useSettingsDrawerStore.getState().width).toBe(
      SETTINGS_DRAWER_MIN_WIDTH
    );
  });
});

describe('toggleSettingsDrawerWith', () => {
  beforeEach(() => {
    useSettingsDrawerStore.setState({ isOpen: false, requestClose: null });
  });

  it('opens the drawer when it is closed', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    await toggleSettingsDrawerWith(open);
    expect(open).toHaveBeenCalledTimes(1);
  });

  it('routes through the guarded close when the drawer is open', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    const requestClose = vi.fn();
    useSettingsDrawerStore.getState().registerCloseRequest(requestClose);
    useSettingsDrawerStore.getState().setOpen(true);

    await toggleSettingsDrawerWith(open);

    expect(requestClose).toHaveBeenCalledTimes(1);
    expect(open).not.toHaveBeenCalled();
  });

  it('opens when marked open but no close request is registered', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    useSettingsDrawerStore.getState().setOpen(true);
    await toggleSettingsDrawerWith(open);
    expect(open).toHaveBeenCalledTimes(1);
  });
});

describe('canDockSettingsDrawer', () => {
  it('docks when the minimum drawer fits beside the app reserve', () => {
    expect(
      canDockSettingsDrawer(
        SETTINGS_DRAWER_MIN_WIDTH + SETTINGS_DRAWER_APP_RESERVE
      )
    ).toBe(true);
  });

  it('falls back to a sheet on narrow desktops', () => {
    expect(canDockSettingsDrawer(800)).toBe(false);
    expect(
      canDockSettingsDrawer(
        SETTINGS_DRAWER_MIN_WIDTH + SETTINGS_DRAWER_APP_RESERVE - 1
      )
    ).toBe(false);
  });
});
