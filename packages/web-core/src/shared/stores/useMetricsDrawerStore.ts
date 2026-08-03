import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/** Narrowest the metrics drawer may be dragged. Mirrors `MetricsDrawer`. */
export const METRICS_DRAWER_MIN_WIDTH = 360;
/** Widest the metrics drawer may be dragged. Mirrors `MetricsDrawer`. */
export const METRICS_DRAWER_MAX_WIDTH = 720;
/** Width used before the operator has ever resized the drawer. */
export const METRICS_DRAWER_DEFAULT_WIDTH = 420;

/** localStorage key for the persisted slice. */
export const METRICS_DRAWER_STORAGE_KEY = 'metrics-drawer';

function clampWidth(width: number) {
  if (!Number.isFinite(width)) return METRICS_DRAWER_DEFAULT_WIDTH;
  return Math.min(
    Math.max(Math.round(width), METRICS_DRAWER_MIN_WIDTH),
    METRICS_DRAWER_MAX_WIDTH
  );
}

export type MetricsDrawerState = {
  /** Whether the server metrics drawer is showing. */
  open: boolean;
  /** Drawer width in pixels, clamped to 360..720. */
  width: number;
  /** Node whose detail is expanded, or `null` for none. */
  selectedNodeId: string | null;
  /** Per-panel collapse state, keyed by panel id (e.g. `'cpu'`). */
  expandedPanels: Record<string, boolean>;

  setOpen: (open: boolean) => void;
  toggleOpen: () => void;
  setWidth: (width: number) => void;
  setSelectedNodeId: (nodeId: string | null) => void;
  setPanelExpanded: (panelId: string, expanded: boolean) => void;
  togglePanel: (panelId: string) => void;
};

/**
 * Persisted view preferences for the server metrics drawer: whether it is
 * open, how wide it is, which node is selected and which panels are expanded
 * — all of which must survive a page reload (FR-13).
 *
 * Deliberately a dedicated store, modelled on `useOrgRailStore`:
 * - `useExpandableStore` is intentionally *not* persisted, so it cannot carry
 *   `expandedPanels` across a reload;
 * - `useUiPreferencesStore` round-trips through a scratch API and a Rust
 *   `UiPreferencesData` type, which is unnecessary weight for local chrome.
 */
export const useMetricsDrawerStore = create<MetricsDrawerState>()(
  persist(
    (set) => ({
      open: false,
      width: METRICS_DRAWER_DEFAULT_WIDTH,
      selectedNodeId: null,
      expandedPanels: {},

      setOpen: (open) => set({ open }),
      toggleOpen: () => set((s) => ({ open: !s.open })),
      setWidth: (width) => set({ width: clampWidth(width) }),
      setSelectedNodeId: (selectedNodeId) => set({ selectedNodeId }),
      setPanelExpanded: (panelId, expanded) =>
        set((s) => ({
          expandedPanels: { ...s.expandedPanels, [panelId]: expanded },
        })),
      togglePanel: (panelId) =>
        set((s) => ({
          expandedPanels: {
            ...s.expandedPanels,
            [panelId]: !s.expandedPanels[panelId],
          },
        })),
    }),
    {
      name: METRICS_DRAWER_STORAGE_KEY,
      partialize: (state) => ({
        open: state.open,
        width: state.width,
        selectedNodeId: state.selectedNodeId,
        expandedPanels: state.expandedPanels,
      }),
      // A hand-edited or stale payload must never yield an unusable width.
      merge: (persisted, current) => {
        const saved = (persisted ?? {}) as Partial<MetricsDrawerState>;
        return {
          ...current,
          ...saved,
          width: clampWidth(saved.width ?? current.width),
          expandedPanels: saved.expandedPanels ?? current.expandedPanels,
        };
      },
    }
  )
);
