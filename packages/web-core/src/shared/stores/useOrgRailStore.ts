import { create } from 'zustand';
import { persist } from 'zustand/middleware';

type State = {
  /** Whether the org section in the AppBar rail is expanded to show all orgs. */
  expanded: boolean;
  toggleExpanded: () => void;
  setExpanded: (value: boolean) => void;
};

/**
 * Persisted expand/collapse state for the organization section of the AppBar
 * rail. Collapsed by default; when expanded the rail shows every organization
 * as an icon tile (see `AppBarOrgTile`). Persisted so the user's choice to
 * "extend" the drawer sticks across reloads.
 */
export const useOrgRailStore = create<State>()(
  persist(
    (set) => ({
      expanded: false,
      toggleExpanded: () => set((s) => ({ expanded: !s.expanded })),
      setExpanded: (value) => set({ expanded: value }),
    }),
    {
      name: 'org-rail-expanded',
      partialize: (state) => ({ expanded: state.expanded }),
    }
  )
);
