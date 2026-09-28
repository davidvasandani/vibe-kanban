# Settings as a docked, non-modal right drawer

Settings (`packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx`)
used to be a centered modal with a backdrop. It is now a right drawer that sits
beside the app and toggles open and closed, so the chat stays usable (constitution XLII).
This page records the design constraints that are not obvious from the code.

## Keep the nice-modal API; change only the frame

About 20 call sites use `SettingsDialog.show(...)`, and some await its promise.
Examples: the gear, `G S`, user popovers, the agent "Customise" link, the repo
setup-script hint, relay pairing links, and the legacy remote org URL. The
modal registration stays. Only the rendered frame changed. The
NiceModalProvider sits at the app root, so the drawer survives route changes
without any extra work.

Rejected alternatives:
- **A section in the workspace `RightSidebar`.** It is workspace-only, a fixed
  300px wide, and shared with the mobile `git` tab
  ([[flexible-collapsible-panel-stacks]]).
- **A `react-resizable-panels` Panel.** Each layout has a different Group, or
  none at all.
- **A floating drawer with no push.** It still covers the chat.

## Docking = app shells reserve the width

`useSettingsDrawerStore` (`shared/stores/`) holds `isOpen`, the persisted
`width` and `requestClose`. `useSettingsDrawerInset(isMobile)` returns the
width to reserve. Both shells apply it as `marginRight` on their desktop root:
`SharedAppLayout` (local) and `RemoteAppShell` (remote). The fixed drawer fills
exactly that gutter. Any new app shell must do the same, or the drawer will
overlap it.

Width rules:
- **Space math decides whether it docks.** The reserve is 720px (the rail,
  the 300px workspaces sidebar and a chat column). The minimum drawer is 520px.
- **Narrow desktops get a sheet.** Below 1240px the drawer becomes a
  full-screen sheet with no push, so the minimum width never squeezes the chat
  to zero. The `md` breakpoint alone is not enough, because the viewport does
  not shrink when the shell does.
- **The workspace right sidebar hides.** `WorkspacesLayout` hides its own 300px
  right sidebar while Settings is open, because Settings takes the right-drawer
  slot. The saved preference is not changed.
- **No transition on the margin.** The layout has to track a live resize drag
  exactly.

## Toggle and guard routing

The open drawer registers its `handleCloseWithConfirmation` as `requestClose`.
`toggleSettingsDrawer()` (implemented by the pure, tested
`toggleSettingsDrawerWith`) opens the drawer, or closes it through that
callback, so every close path (X, gear, `G S`, Escape) hits the same
unsaved-changes guard. `Actions.Settings.isActive` reads
`ctx.isSettingsOpen`, so the gear shows as filled while the drawer is open.

## Deep links while open: stamp each request

nice-modal re-renders the mounted component with the new `show()` args.
Comparing props is not enough: repeating the same link after navigating
inside Settings produces identical props. `SettingsDialog.show` is wrapped to
stamp a monotonically increasing `requestId`, and the drawer re-targets on
each new id. Re-targeting away from dirty edits runs the same discard
confirmation first. A section's cleanup clears its dirty flag on unmount, so
an unguarded switch would lose the edits silently.

## Layering and Escape

The desktop drawer uses `z-[90]`. That is above page chrome (≤ 50) and below
`@vibe/ui` Dialog (9998/9999) and dropdowns (10000), so confirmations and
selects opened from Settings still stack on top. Escape is handled by
`onKeyDown` on the drawer, and only for targets DOM-contained in it. Escape in
the chat, or in a Radix menu portaled out of the drawer, does not close
Settings. The drawer focuses itself when it opens.

## Contributed by

- vk/4643-move-settings-to
