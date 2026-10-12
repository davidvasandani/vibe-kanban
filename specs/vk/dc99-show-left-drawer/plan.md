# Implementation Plan: Show the left drawer until there is no space

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- Frontend only, in `packages/web-core` (React 18, TypeScript, zustand,
  react-resizable-panels v2+, Vitest for pure-logic tests). The local-web and
  remote-web shells both consume it (constitution IV).
- There are no backend, scratch or generated-type changes. The saved
  preference `is_left_sidebar_visible` keeps its meaning (wiki
  workspace-sidebar-filtering: the typed scratch drops frontend-only fields,
  and that is what we want for the new transient state).

## Architecture & Approach
1. **Fit rule (FR-1, FR-2)**: a new pure module,
   `packages/web-core/src/shared/lib/workspaceLayoutFit.ts`, exports the width
   constants and `leftSidebarFits(input)`. See `contracts/workspaceLayoutFit.md`.
2. **Store (FR-4, FR-6)**: changes in
   `packages/web-core/src/shared/stores/useUiPreferencesStore.ts`.
   - Remove `isWideScreen` and the two `isLeftSidebarVisible` writes in
     `toggleRightMainPanelMode` and `setRightMainPanelMode`.
   - Add the transient `leftSidebarHasSpace` and `isLeftSidebarForcedOpen`,
     and `setLeftSidebarHasSpace`.
   - Export `selectIsLeftSidebarShown`.
   - Rewrite `toggleLeftSidebar` to act on the shown state.
   - `useWorkspacePanelState` returns the shown value as
     `isLeftSidebarVisible`. Its consumers (`WorkspacesLayout` rendering and
     `useActionVisibilityContext` → `ToggleLeftSidebar.isActive`) then follow
     it without further edits.
3. **Measurement (FR-3)**: changes in
   `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx`.
   - Attach a callback ref to the desktop root `div.flex.flex-1`, observed
     with `ResizeObserver` and stored in `useState<number | null>`.
   - Compute `leftSidebarFits` from `isLeftMainPanelVisible`,
     `rightMainPanelMode !== null` and the rendered right sidebar
     (`isRightSidebarVisible && !isCreateMode && !isSettingsDrawerOpen`).
     The Settings inset is already outside the root, because the shell
     applies it as margin.
   - Publish the result with `setLeftSidebarHasSpace` in an effect, and reset
     it to `true` on unmount.
   - The mobile branch never attaches the ref (FR-8).
   - The existing "restore drawer when right panel closes" effect stays
     (FR-7).
4. **Reopen affordance (FR-5)**: in
   `packages/web-core/src/pages/root/SharedAppLayout.tsx`, `isLeftSidebarVisible`
   is read through `selectIsLeftSidebarShown`, so the hover tag and preview
   turn on whenever the drawer is not on screen.
5. **Action label (FR-6)**: in `packages/web-core/src/shared/actions/index.ts`,
   `ToggleLeftSidebar.label` uses `selectIsLeftSidebarShown(getState())`.

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/workspaceLayoutFit.md`.

## Research Notes
See `./research.md`. No new dependency is added: `ResizeObserver` is native
and already used in `PreviewBrowserContainer.tsx`.

## Constitution Check
- **LIV**: honoured directly. The plan measures the host area, uses a pure
  tested helper with named constants, keeps hiding derived and unpersisted,
  shares one "shown" selector, and limits the override to a transient one.
- **III / VI**: the plan extends the existing store, toggle, hover preview
  and restore effect. No new layout system is added.
- **IV**: the change is confined to web-core containers and stores. No
  `@vibe/ui` presentation change.
- **XLII**: the Settings drawer still takes the right-sidebar slot, and the
  fit rule counts only the sidebar that is actually rendered.
- **II**: there are Vitest tests for the fit rule and the store transitions.
  The layout wiring is checked manually in the dev app.

No deviations.

## Risks & Dependencies
- **Flash on first paint in a narrow window**: the width is `null` until the
  first observation, so the drawer is treated as fitting. ResizeObserver
  fires before the first paint in practice, which keeps the flash to at most
  one frame. This is accepted.
- **Users with a stale saved `false`**: they recover by closing a right panel
  or with one toggle (clarification Q2).
- **Oscillation**: the measured root contains the drawer, so showing or
  hiding the drawer does not change the measured width. The rule is stable.
