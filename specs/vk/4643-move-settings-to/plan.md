# Implementation Plan: Settings as a right drawer

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
- **Code:** a frontend-only change in `packages/web-core`, which is shared by
  local-web and remote-web (constitution IV: both frontends are the blast
  radius), plus two app-shell files in `packages/web-core` and
  `packages/remote-web`. No backend, API or type-generation changes.
- **Libraries already in use:**
  - React 18, Tailwind
  - `@ebay/nice-modal-react`: Settings is registered through `defineModal` in
    `web-core/src/shared/lib/modals.ts`
  - zustand: the pattern in `web-core/src/shared/stores/*.ts`
  - vitest
- **Mobile breakpoint:** `useIsMobile()` (`max-width: 767px`), the same one the
  layouts already use.
- **No new dependencies.** Resizing uses plain pointer events. The layouts'
  `react-resizable-panels` would need the drawer inside each layout's Group,
  which is too invasive (see research).

## Architecture & Approach
1. **Drawer state store:** new file
   `web-core/src/shared/stores/useSettingsDrawerStore.ts` (data-model.md).
   - It holds `isOpen`, the persisted `width`, and a `requestClose` callback
     registered by the mounted drawer.
   - It exports the pure `clampSettingsDrawerWidth()` and
     `useSettingsDrawerInset(isMobile)`.
2. **Drawer chrome:** `web-core/src/shared/dialogs/settings/SettingsDialog.tsx`.
   - `SettingsDialogContent` keeps its navigation, section rendering, the dirty
     and host providers, and `handleCloseWithConfirmation`. Only the frame
     changes:
     - Remove the overlay `div` (FR-1, FR-3).
     - Desktop frame: `fixed inset-y-0 right-0 z-[90]` with inline `width`
       from the store, `border-l`, `bg-panel`, and a slide-in animation.
     - Mobile frame: unchanged, `fixed inset-0`.
     - A 6px `cursor-col-resize` handle on the left edge (desktop). It uses
       pointer capture and calls `setWidth(clamp(window.innerWidth -
       clientX))` (FR-7).
     - The nav column goes from `md:w-56` to `md:w-48` to leave room for
       section content.
   - Replace the `window` Escape listener with an `onKeyDown` on the drawer
     root that skips `e.defaultPrevented` (Radix menus handle their own
     Escape) (FR-5).
   - On mount, call `setOpen(true)` and `registerCloseRequest(
     handleCloseWithConfirmation)`. On unmount, clear both.
   - Re-show while open (FR-8): nice-modal passes the new `show()` args as new
     props to the mounted component.
     - When `initialSection` or `initialState` changes, set `activeSection` and
       `mobileShowContent`.
     - If the requested section is already active and nothing is dirty, bump
       `sectionKey` so the section remounts with the new `initialState`.
3. **Toggle:** export `toggleSettingsDrawer(props?)` from `SettingsDialog.tsx`.
   If the store is open, it calls `requestClose()`. Otherwise it calls
   `SettingsDialog.show(props)` (FR-4, FR-6). Callers:
   - `web-core/src/shared/actions/index.ts`: `Actions.Settings.execute` calls
     the toggle, and a new `isActive: (ctx) => ctx.isSettingsOpen` lights the
     gear.
   - `web-core/src/shared/types/actions.ts`: add `isSettingsOpen: boolean` to
     `ActionVisibilityContext`.
   - `web-core/src/shared/hooks/useActionVisibilityContext.ts`: read
     `useSettingsDrawerStore((s) => s.isOpen)`.
   - `remote-web/src/app/providers/RemoteActionsProvider.tsx`: the `settings`
     override calls `toggleSettingsDrawer({ initialSection: 'organizations' })`.
   - `remote-web/src/app/layout/RemoteAppShell.tsx` `handleOpenSettings` and
     `remote-web/src/app/layout/RemoteNavbarContainer.tsx`
     `handleOpenSettings` call `toggleSettingsDrawer()`.
   - Deep links (agents, repos, relay, organizations, remote-projects) keep
     calling `SettingsDialog.show(...)` and get FR-8 behavior (FR-11).
4. **Layout push (FR-2):** `const inset = useSettingsDrawerInset(isMobile)`.
   - `web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx`:
     desktop root grid gets `style={{ marginRight: inset }}`.
   - `remote-web/src/app/layout/RemoteAppShell.tsx`: root div gets the same.
   - The fixed drawer occupies exactly that gutter. Popovers inside the app
     stay positioned relative to the viewport, which is unaffected.
5. **Stacking (FR-9):** use `z-[90]`. That is below `@vibe/ui` Dialog
   (9998/9999) and dropdowns (10000), so the ConfirmDialog and SettingsSelect
   menus stay on top. It is above the page content (z ≤ 50).
6. **Persistence across navigation (FR-12):** comes for free. The
   NiceModalProvider sits at the app root (`local-web/src/routes/_app.tsx`,
   `remote-web/src/routes/__root.tsx`), so route changes do not unmount the
   drawer.

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/settings-drawer.md`.

## Research Notes
See `./research.md`.

## Constitution Check
- **XLII** (auxiliary surfaces do not block primary work): all clauses map to
  FR-1 to FR-11. There is no overlay. The drawer is docked with a layout push,
  has a single toggle with an active state, keeps the unsaved-changes guard on
  every path, uses focus-scoped Escape, sits below dialogs, retargets on deep
  links, and keeps the `show()` signature.
- **III / VI** (small steps, don't rebuild): reuses nice-modal, the existing
  Settings internals and the dirty guard. No new libraries.
- **IV:** the change lives in web-core. The remote shell and local shell both
  apply the inset, so both frontends are covered.
- **II:** vitest unit tests for the clamp and the store's toggle/close routing.
  There is no existing rendered-DOM test for SettingsDialog to extend.
- **Constraints:** no generated files are touched. `pnpm run format` runs
  before completion.
- No deviations.

## Risks & Dependencies
- **Layouts without the inset:** a route rendered outside
  `SharedAppLayout`/`RemoteAppShell` (for example onboarding or the vscode
  embed) gets the drawer over its right edge. That is acceptable: it is no
  worse than the previous full-screen modal.
- **Re-show prop identity:** nice-modal hands new props on each `show()`.
  Comparing `initialState` by JSON avoids remount loops.
- **Narrow desktop windows:** the clamp keeps at least 720px for the app. Below
  that the layout is cramped but usable, and mobile mode takes over at < 768px.

## Post-review amendments
- `SettingsDialog.show` is wrapped to stamp `requestId`. The content re-targets
  on each new `requestId` and runs `confirmDiscardIfDirty` first.
- `WorkspacesLayout.tsx` hides the desktop workspace right sidebar while
  `useSettingsDrawerStore.isOpen`. The reserve is raised to 720px.
- `canDockSettingsDrawer(viewport)` / `useSettingsDrawerLayout(isMobile)`: dock
  only when `viewport ≥ MIN + RESERVE` (1240px). Otherwise the drawer renders
  as a full-screen sheet and the inset is 0.
