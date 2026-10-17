# Tasks: Settings as a right drawer

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup
- [x] T001 [P] Create the drawer store in
  `packages/web-core/src/shared/stores/useSettingsDrawerStore.ts`:
  - fields `isOpen`, `width` (localStorage `vibe.ui.settingsDrawerWidth`) and
    `requestClose`
  - `setOpen`, `setWidth`, `registerCloseRequest`
  - bounds constants, `clampSettingsDrawerWidth` and `useSettingsDrawerInset`
- [x] T002 [P] Add `isSettingsOpen: boolean` to `ActionVisibilityContext` in
  `packages/web-core/src/shared/types/actions.ts`

## Phase 2: Core
- [x] T003 Rework the frame of `SettingsDialogContent` in
  `packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx` (depends
  on T001):
  - remove the overlay
  - fixed right drawer at `z-[90]` using the clamped store width
  - left-edge pointer resize handle
  - `md:w-48` nav
  - drawer-scoped Escape
  - register/unregister `isOpen` and `requestClose`
  - retarget on changed `initialSection`/`initialState`, with a `sectionKey`
    bump
  - export `toggleSettingsDrawer(props?)`
- [x] T004 [P] Read `useSettingsDrawerStore((s) => s.isOpen)` into
  `isSettingsOpen` in
  `packages/web-core/src/shared/hooks/useActionVisibilityContext.ts` (depends
  on T001, T002)
- [x] T005 In `Actions.Settings` (`packages/web-core/src/shared/actions/index.ts`),
  execute through `toggleSettingsDrawer()` and add `isActive: (ctx) =>
  ctx.isSettingsOpen` (depends on T002, T003)

## Phase 3: Integration
- [x] T006 [P] Apply `marginRight: useSettingsDrawerInset(isMobile)` (no
  transition, so the layout tracks a live resize drag) to the desktop root in
  `packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx`
  (depends on T001)
- [x] T007 [P] In `packages/remote-web/src/app/layout/RemoteAppShell.tsx`, apply
  the same inset to the root, and have `handleOpenSettings` call
  `toggleSettingsDrawer()` (depends on T001, T003)
- [x] T008 [P] Have `handleOpenSettings` call `toggleSettingsDrawer()` in
  `packages/remote-web/src/app/layout/RemoteNavbarContainer.tsx` (depends on
  T003)
- [x] T009 [P] Change the `settings` override to
  `toggleSettingsDrawer({ initialSection: 'organizations' })` in
  `packages/remote-web/src/app/providers/RemoteActionsProvider.tsx` (depends on
  T003)

## Phase 4: Validation
- [x] T010 [P] Add unit tests in
  `packages/web-core/src/shared/stores/useSettingsDrawerStore.test.ts`:
  - clamp bounds (min, max, viewport reserve, tiny viewport)
  - width persistence and default
  - `requestClose` registration
  - `toggleSettingsDrawerWith` routing: closed → show, open → requestClose
- [x] T011 Run web-core and remote-web vitest, `pnpm run check`,
  `pnpm run lint` and `pnpm run format`. Fix any fallout (depends on
  T001–T010). Frontend parts only: no Rust changed, so backend check
  and clippy were skipped
- [ ] T012 Check in the browser: the drawer pushes the layout, the chat stays
  usable, the gear toggles, and width persists (depends on T011). NOT RUN: it
  needs a locally built Rust backend, which this worktree does not have
