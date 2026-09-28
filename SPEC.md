# SPEC — Move Settings to a right drawer

Task: `vk/4643-move-settings-to`

## Problem

Settings opens as a centered 900×700 modal with a dimmed full-screen overlay
(`packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx`). While it is
open the chat, the workspace sidebar and the right sidebar are covered and
unclickable. Clicking outside closes it, and so does Escape pressed anywhere.
The operator cannot keep Settings open while reading or typing in the chat.

## Goal

Settings becomes a right-side drawer. You toggle it open and closed, and it sits
next to the app instead of on top of it:

1. **No overlay.** Nothing dims or blocks the rest of the app. Clicking outside
   the drawer does not close it.
2. **Docked, not floating (desktop, ≥ md).** The drawer covers the full height at
   the right edge. The app shell (navbar + content) shrinks by the drawer's
   width, so the chat and panels reflow and stay fully visible and usable.
3. **Toggle.** The navbar gear (`Actions.Settings`, shortcut `G S`) opens the
   drawer when it is closed and closes it when it is open. The gear shows as
   active (filled) while the drawer is open. The drawer's own X also closes it.
4. **Resizable.** Dragging the drawer's left edge sets its width, clamped to
   [520px, viewport − 720px, max 1200px]. The default is 720px. The width is
   kept in `localStorage` (`vibe.ui.settingsDrawerWidth`).
5. **Unsaved changes are still protected.** Every close path (X, gear toggle,
   `G S`, Escape) goes through the existing dirty-state confirmation.
6. **Escape is scoped.** Escape closes the drawer only when focus is inside the
   drawer, so pressing Escape in the chat does not close Settings.
7. **Deep links while open.** A `SettingsDialog.show({ initialSection, ... })`
   call made while the drawer is already open (for example the agent
   "Customise" link, a repo setup-script hint, or the relay pairing link) goes
   to the requested section instead of being ignored.
8. **Mobile (< md) is unchanged.** It stays a full-screen sheet with the
   nav/content toggle and a back button. The layout is not pushed.

## Non-goals

- Changing any settings section's content, saving behavior, or the machine
  (host) picker.
- Converting Settings into URL routes.
- Moving Settings into the workspace `RightSidebar` section stack. That sidebar
  is 300px, workspace-only, and also used as the mobile tab. Settings has to
  work from every route (kanban, remote home), so it is an app-shell drawer.

## Design

- **Keep the public API.** `SettingsDialog.show(props)` / `.hide()` stay
  nice-modal based, so all ~20 call sites keep working. The promise still
  resolves when the drawer closes.
- **New store** `packages/web-core/src/shared/stores/useSettingsDrawerStore.ts`
  (zustand):
  - `isOpen: boolean`
  - `width: number` (persisted)
  - `setWidth(px)`
  - `requestClose: (() => void) | null`, registered by the open drawer so a
    toggle can route through the dirty-check
  - `setOpen`, `registerCloseRequest`
  - Pure helper `clampSettingsDrawerWidth(width, viewportWidth)`
- **Toggle** helper `toggleSettingsDrawer()` in `SettingsDialog.tsx`: if open,
  call `requestClose()`. Otherwise call `SettingsDialog.show()`.
  `Actions.Settings.execute` uses it. `Actions.Settings.isActive` reads the
  store's `isOpen`. This needs `isSettingsOpen` in the action visibility
  context.
- **Layout inset** hook `useSettingsDrawerInset()`: returns the drawer width
  when open on desktop, otherwise 0. `SharedAppLayout` (local) and
  `RemoteAppShell` (remote) apply it as `marginRight` on their desktop root.
  The fixed drawer fills exactly that gutter.
- **Rendering.** The drawer is portaled to `document.body` and positioned
  `fixed inset-y-0 right-0` at `z-[90]`. That is below Dialogs (9998/9999) and
  dropdowns (10000), so confirm dialogs and selects opened from Settings still
  appear on top. It has a `border-l` and `bg-panel`. Inside, the settings nav
  column is narrower (`md:w-48`).
- **Re-show while open.** Watch the `initialSection`/`initialState` props. When
  they change, select the requested section. If it is the same section with
  new state and no unsaved edits, remount the section (key bump) so the new
  state applies.

## Acceptance criteria

- With Settings open on desktop, the chat input can be focused, typed in, and
  sent. The workspace sidebar, right sidebar and navbar all respond to clicks.
- The gear and `G S` toggle the drawer. The gear is filled while it is open.
- Dirty sections still prompt before closing, whatever the close path.
- The width can be dragged, is clamped, and is still set after a reload.
- Mobile behaves as before.
- `pnpm run check`, `pnpm run lint` and web-core vitest pass. There are unit
  tests for the width clamp and the store's toggle/close logic.

## Review follow-ups (Codex)

- **Deep-link guard:** a deep link that would leave a section with unsaved
  edits now goes through the same discard confirmation as closing.
- **Repeated deep links:** each `SettingsDialog.show()` is stamped with a
  `requestId`, so repeating the same link after moving around inside Settings
  still re-targets the drawer.
- **Narrow desktops:** the workspace's own 300px right sidebar is hidden while
  Settings is open, because Settings takes the right-drawer slot. The saved
  preference is not changed. The app reserve is raised to 720px (the rail,
  the 300px workspaces sidebar and a chat column), so the chat keeps a usable
  width at about 1280px.
- **Too narrow to dock:** below 1240px (520px minimum drawer plus 720px app
  reserve), Settings falls back to a full-screen sheet with no layout push, as
  on mobile. It docks only when both columns fit.
