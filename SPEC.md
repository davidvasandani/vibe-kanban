# SPEC: Show the left drawer until there is no space

Task: `vk/dc99-show-left-drawer`. Feature spec, plan and tasks:
`specs/vk/dc99-show-left-drawer/`.

## Problem

In the desktop workspace view (`/workspaces/:id`) the left drawer (the 300px
workspaces list, `WorkspacesSidebarContainer`) disappears even when the window
has plenty of room for it. The screen recording shows a ~1300px-wide window
with only the chat open: the drawer stays hidden (only the hover "reopen" tag
is visible), and widening the window does not bring it back.

### Root cause

`useUiPreferencesStore` (`packages/web-core/src/shared/stores/useUiPreferencesStore.ts`)
collapses the drawer with a fixed, one-shot breakpoint:

```ts
const isWideScreen = () => window.innerWidth > 2048;
```

- `toggleRightMainPanelMode` / `setRightMainPanelMode` write
  `isLeftSidebarVisible = false` whenever a right main panel (Changes, Logs,
  Preview, Browser) opens on any window ≤ 2048px — i.e. almost every laptop
  and most desktop monitors, regardless of how much room is actually free.
- The value is read once, at the moment the panel opens. Resizing the window
  never re-evaluates it.
- `isLeftSidebarVisible` is the *persisted user preference* (synced to the
  server scratch as `is_left_sidebar_visible`), so the automatic collapse is
  saved as if the user had hidden the drawer and survives reloads. The
  `WorkspacesLayout` effect that restores the drawer when the right panel
  closes only runs when its deps change, so a reload that hydrates the stale
  `false` after mount leaves the drawer hidden with nothing open — the state
  in the recording.

## Goal

The left drawer is shown whenever the layout has room for it next to the
content that is open, and is hidden automatically only when it does not fit.
The decision follows the live width of the workspace area, so it reacts to
window resizes, opening/closing panels and toggling the right sidebar.

## Requirements

- R1 `isLeftSidebarVisible` means only the user's intent. Opening or closing
  a right main panel no longer writes it; `isWideScreen` is removed.
- R2 The workspace layout measures its own width (ResizeObserver) and computes
  whether the drawer fits:
  `width ≥ LEFT_SIDEBAR_WIDTH + (right sidebar shown ? RIGHT_SIDEBAR_WIDTH : 0)
  + MIN_CHAT_WIDTH + (right main panel open ? MIN_RIGHT_MAIN_PANEL_WIDTH : 0)`
  (both sidebars 300px; chat min 480px; right main panel min 400px). Only the
  panels that are actually rendered count (e.g. a hidden chat panel does not).
- R3 The drawer is rendered when the user wants it **and** it fits. The result
  is published to the store (non-persisted) so everything that depends on the
  drawer being on screen uses the same answer: the hover reopen tag/preview in
  `SharedAppLayout`, the "Show/Hide Left Sidebar" action label and active
  state, and `useWorkspacePanelState`.
- R4 Toggling the drawer (`V S`, navbar, command bar) acts on what is on
  screen: visible → hide (preference false); hidden by preference → show
  (preference true); hidden for lack of space → show it anyway (a transient,
  non-persisted override). The override ends when the space situation changes
  or the drawer is toggled off.
- R5 Mobile layout (≤ 767px) is unchanged; the measurement is desktop-only.
- R6 Stale persisted `false` written by the old auto-collapse is not
  distinguishable from a real user choice; the existing "restore drawer when
  the right panel closes" behaviour is kept so closing a panel still brings
  it back.
- R7 Unit tests (Vitest) cover the fit calculation and the store's toggle /
  override / right-panel behaviour.

## Non-goals

- Making the drawer resizable or changing its 300px width.
- Auto-hiding the right sidebar.
- Changing the mobile tabs layout or the hover preview's look.

## Acceptance criteria

- AC1 With chat only, right sidebar shown, a window ≥ ~1210px of workspace
  area shows the drawer; shrinking below hides it; widening again shows it
  without any click.
- AC2 Opening Changes on a 1920px screen keeps the drawer; on a 1440px laptop
  with the right sidebar open it hides, and reappears when the right sidebar
  or the panel is closed.
- AC3 Opening a right panel never changes the persisted
  `is_left_sidebar_visible`.
- AC4 When the drawer is auto-hidden, the hover reopen tag works and `V S`
  shows the drawer.
- AC5 `pnpm run check`, `pnpm run lint`, web-core Vitest pass.
