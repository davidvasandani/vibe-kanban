import { useEffect, useState } from 'react';
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export const SETTINGS_DRAWER_DEFAULT_WIDTH = 720;
export const SETTINGS_DRAWER_MIN_WIDTH = 520;
export const SETTINGS_DRAWER_MAX_WIDTH = 1200;
/**
 * Pixels always left for the app shell beside the drawer: the rail, the
 * 300px workspaces sidebar and a readable chat column.
 */
export const SETTINGS_DRAWER_APP_RESERVE = 720;

export function clampSettingsDrawerWidth(
  width: number,
  viewportWidth: number
): number {
  const max = Math.min(
    SETTINGS_DRAWER_MAX_WIDTH,
    viewportWidth - SETTINGS_DRAWER_APP_RESERVE
  );
  return Math.round(Math.max(SETTINGS_DRAWER_MIN_WIDTH, Math.min(width, max)));
}

type CloseRequest = () => void | Promise<void>;

type State = {
  /** True while the Settings drawer is mounted and visible. */
  isOpen: boolean;
  /** Preferred width in px; clamp against the viewport when rendering. */
  width: number;
  /** Close through the drawer's unsaved-changes guard. */
  requestClose: CloseRequest | null;
  setOpen: (open: boolean) => void;
  setWidth: (width: number) => void;
  registerCloseRequest: (fn: CloseRequest | null) => void;
};

/**
 * Settings is a non-modal right drawer (see SettingsDialog). This store lets
 * the app shells reserve its width and lets the gear toggle it closed through
 * the same unsaved-changes guard as the drawer's own close button. Only the
 * width is persisted; a reload starts with Settings closed.
 */
export const useSettingsDrawerStore = create<State>()(
  persist(
    (set) => ({
      isOpen: false,
      width: SETTINGS_DRAWER_DEFAULT_WIDTH,
      requestClose: null,
      setOpen: (open) => set({ isOpen: open }),
      setWidth: (width) =>
        set({ width: Math.max(SETTINGS_DRAWER_MIN_WIDTH, Math.round(width)) }),
      registerCloseRequest: (fn) => set({ requestClose: fn }),
    }),
    {
      name: 'vibe.ui.settingsDrawerWidth',
      partialize: (state) => ({ width: state.width }),
    }
  )
);

function useViewportWidth(): number {
  const [viewportWidth, setViewportWidth] = useState(() =>
    typeof window === 'undefined' ? Infinity : window.innerWidth
  );
  useEffect(() => {
    const handleResize = () => setViewportWidth(window.innerWidth);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);
  return viewportWidth;
}

/**
 * Whether the viewport fits the minimum drawer beside the app reserve. Below
 * this, Settings falls back to a full-screen sheet instead of squeezing the
 * app to nothing.
 */
export function canDockSettingsDrawer(viewportWidth: number): boolean {
  return (
    viewportWidth >= SETTINGS_DRAWER_MIN_WIDTH + SETTINGS_DRAWER_APP_RESERVE
  );
}

/** Docked state and clamped width of the drawer for the current viewport. */
export function useSettingsDrawerLayout(isMobile: boolean): {
  docked: boolean;
  width: number;
} {
  const width = useSettingsDrawerStore((s) => s.width);
  const viewportWidth = useViewportWidth();
  return {
    docked: !isMobile && canDockSettingsDrawer(viewportWidth),
    width: clampSettingsDrawerWidth(width, viewportWidth),
  };
}

/**
 * Pixels an app shell must reserve on its right edge so the docked Settings
 * drawer sits beside it rather than over it. 0 when closed or when Settings is
 * a full-screen sheet (mobile or a desktop too narrow to dock).
 */
export function useSettingsDrawerInset(isMobile: boolean): number {
  const isOpen = useSettingsDrawerStore((s) => s.isOpen);
  const { docked, width } = useSettingsDrawerLayout(isMobile);
  return isOpen && docked ? width : 0;
}

/**
 * Toggle semantics shared by the gear and `G S`: route through the open
 * drawer's unsaved-changes guard when it is open, otherwise open it.
 */
export async function toggleSettingsDrawerWith(
  open: () => Promise<void>
): Promise<void> {
  const { isOpen, requestClose } = useSettingsDrawerStore.getState();
  if (isOpen && requestClose) {
    await requestClose();
    return;
  }
  await open();
}
