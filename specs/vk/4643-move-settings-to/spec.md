# Feature Specification: Settings as a right drawer

**Feature dir**: `specs/vk/4643-move-settings-to/`
**Status**: Draft

## Summary
Settings opens today as a centered dialog over a dimmed backdrop. While it is
open, the chat, sidebars and navbar cannot be used, and clicking anywhere
outside it closes it. Operators often want to change a setting while watching
or talking to an agent (switch an agent's model, check a machine's MCP servers,
look up a repo's setup script). This feature turns Settings into a right-side
drawer. You toggle it open and closed, and it sits beside the app, so the chat
stays visible and usable while Settings is open (constitution XLII).

## User Stories
- As an operator, I want Settings to open beside the chat so that I can keep
  reading and replying to the agent while I adjust configuration.
- As an operator, I want the same gear button (and `G S`) to open and close
  Settings so that I can dismiss it without looking for a close button.
- As an operator, I want to size the drawer so that wide settings pages
  (MCP servers, pipelines) have room when I need it, and the chat does when I
  don't.
- As an operator, I want unsaved Settings edits protected however I close the
  drawer so that a toggle never silently throws work away.
- As a mobile user, I want Settings to keep working as a full-screen sheet so
  that nothing gets cramped on a phone.

## Functional Requirements
- FR-1: On desktop-width screens, Settings appears as a full-height panel
  docked at the right edge of the window. No backdrop dims or blocks the rest
  of the app.
- FR-2: While the drawer is open, the rest of the app (navbar, sidebars, chat)
  narrows to fit beside it rather than being covered, and stays fully usable.
- FR-3: Clicking outside the drawer does not close it.
- FR-4: The Settings gear and the `G S` shortcut toggle the drawer: they open
  it when it is closed and close it when it is open. The gear shows as active
  while the drawer is open.
- FR-5: The drawer has a close control. Escape closes the drawer only when
  keyboard focus is inside it.
- FR-6: Every way of closing the drawer asks for confirmation first if any
  Settings section has unsaved changes, exactly as the dialog did.
- FR-7: The user can resize the drawer by dragging its inner (left) edge,
  within limits that always leave room for the app (default 720px, minimum
  520px, maximum min(1200px, window − 720px)). The chosen width is remembered
  across reloads. The open/closed state is not.
- FR-12: The drawer stays open across in-app navigation.
- FR-8: When something asks to open a specific Settings section while the
  drawer is already open (for example the agent "Customise" link, a repository
  setup-script hint, or a pairing link), the drawer switches to that section.
- FR-9: Dialogs, confirmations and dropdowns opened from within Settings still
  appear above the drawer.
- FR-10: On narrow (mobile) screens, Settings keeps its current full-screen
  behavior.
- FR-11: Existing entry points to Settings (gear, keyboard shortcut, user
  popover, deep links) keep working without being rewritten.

## Out of Scope
- Changing the content, save behavior or machine picker of any Settings
  section.
- Addressable URLs for Settings sections.
- Folding Settings into the workspace's existing right sidebar sections.

## Acceptance Criteria
- [ ] With the drawer open on desktop, the user can click into the chat input,
      type a message and send it.
- [ ] Opening the drawer narrows the app beside it. Closing it restores the
      full width.
- [ ] The gear and `G S` open the drawer when it is closed and close it when it
      is open. The gear is shown as active while it is open.
- [ ] With an unsaved edit, closing by the gear, `G S`, the X or Escape (focus
      inside) shows the discard confirmation. Cancelling keeps the drawer and
      the edit.
- [ ] Pressing Escape while typing in the chat does not close the drawer.
- [ ] Dragging the edge resizes the drawer within limits, and the width is
      still set after a reload.
- [ ] Clicking the agent "Customise" link while the drawer is open on General
      switches it to Agents.
- [ ] On a phone-width viewport, Settings still opens as a full-screen sheet.

## Clarifications

No answers were supplied with the task, so each question below was settled on
the least surprising behavior. Each one can be revisited without redesign.

- **Route changes:** the drawer **stays open** when the user navigates, for
  example from a workspace to the kanban board. It belongs to the app shell,
  not to a page, and the operator opened it on purpose. Unsaved edits are
  never dropped silently by navigation (FR-6 still governs explicit closes).
- **Reload:** only the **width** is remembered. The open/closed state is not.
  A reload starts with Settings closed, as the dialog did. This avoids
  reopening a half-finished edit whose draft was lost on reload.
- **Width:** the default is **720px** and the minimum is **520px**. The
  maximum is the smaller of 1200px and the window width minus 720px, so the
  app keeps a usable column. A remembered width that no longer fits (for
  example on a smaller window) is clamped when shown, and the saved value is
  left unchanged.

## Open Questions
None remaining.
