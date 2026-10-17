# Research: Settings as a right drawer

## Decision: an app-shell drawer on top of the existing nice-modal registration
- **Chosen:** keep `SettingsDialog` as a nice-modal component and change only
  its frame into a fixed right drawer. Layouts reserve the drawer's width with
  `margin-right`.
- **Why:** there are about 20 call sites (some awaiting the close promise), and
  they keep working unchanged (FR-11). The drawer survives navigation because
  the modal provider is at the app root.
- **Rejected: a section in `RightSidebar`.** It is workspace-only, a fixed
  300px, and shared with the mobile `git` tab
  (`wiki/flexible-collapsible-panel-stacks.md`). Settings must also open from
  kanban and remote home.
- **Rejected: a Panel inside `react-resizable-panels`.** Each layout
  (`WorkspacesLayout`, kanban, remote pages) has a different Group or none at
  all. Threading a Settings panel into each would multiply the blast radius.
- **Rejected: a floating drawer with no push.** It would still hide part of the
  chat at typical widths, which conflicts with the request "so it doesn't block
  the chat".

## Decision: pointer-event resize, width stored in localStorage
- The resize needs no dependency. The width is a per-browser preference like
  the `vibe.ui.collapsible.*` keys, so it lives in localStorage under
  `vibe.ui.settingsDrawerWidth` rather than the server UI-preferences scratch.
  That keeps it off the scratch schema.

## Decision: z-index 90
- In `@vibe/ui`, Dialog uses 9998/9999 and dropdowns/popovers 10000. The restart
  banner uses 100. At 90 the drawer sits above page chrome and below every
  modal or menu, including the ones Settings opens itself.

## Decision: Escape scoped to focus
- The old `window` listener closed Settings on any Escape. Now that the chat
  stays interactive, Escape in the composer (used to cancel or blur) must not
  close Settings, so the drawer root handles `onKeyDown` instead.
