# Tasks: Show the left drawer until there is no space

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Foundations
- [x] T001 [P] Add width constants, `requiredWidthForLeftSidebar` and
  `leftSidebarFits` in
  `packages/web-core/src/shared/lib/workspaceLayoutFit.ts` (FR-1, FR-2)
- [x] T002 [P] Store changes in
  `packages/web-core/src/shared/stores/useUiPreferencesStore.ts` (FR-4, FR-6):
  - remove `isWideScreen` and the right-panel writes to `isLeftSidebarVisible`;
  - add `leftSidebarHasSpace`, `isLeftSidebarForcedOpen`,
    `setLeftSidebarHasSpace` and `selectIsLeftSidebarShown`;
  - make the `toggleLeftSidebar` transitions follow `data-model.md`;
  - make `useWorkspacePanelState` return the shown value.

## Phase 2: Wiring (depends on T001, T002)
- [x] T003 Measure the desktop root with `ResizeObserver`, compute the fit
  from the rendered panels, publish it, and reset it on unmount, in
  `packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx` (FR-3, FR-7,
  FR-8)
- [x] T004 [P] Turn the hover reopen tag and preview on when the drawer is not
  shown, in `packages/web-core/src/pages/root/SharedAppLayout.tsx` (FR-5)
- [x] T005 [P] Make the `ToggleLeftSidebar` label follow the shown state in
  `packages/web-core/src/shared/actions/index.ts` (FR-6)

## Phase 3: Validation
- [x] T006 [P] Unit tests for the fit rule in
  `packages/web-core/src/shared/lib/workspaceLayoutFit.test.ts` (depends on
  T001)
- [x] T007 [P] Unit tests for the store transitions (right panel keeps the
  preference, three toggle states, override cleared on space change) in
  `packages/web-core/src/shared/stores/useUiPreferencesStore.leftSidebar.test.ts`
  (depends on T002)
- [x] T008 Run `pnpm install --frozen-lockfile`, web-core `vitest`,
  `pnpm run check`, `pnpm run lint` and `pnpm run format`, then check the
  layout in the dev app at several widths (depends on T003–T007)

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
