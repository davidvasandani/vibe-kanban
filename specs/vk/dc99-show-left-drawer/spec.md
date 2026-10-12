# Feature Specification: Show the left drawer until there is no space

**Feature dir**: `specs/vk/dc99-show-left-drawer/`
**Status**: Draft

## Summary
In the desktop workspace view, the left drawer (the workspaces list) is often
hidden even when the window has plenty of room. The user's screen recording
shows a mid-sized window with only the chat open. The drawer is hidden, only
the hover reopen tag is visible, and resizing the window does not bring it
back. The drawer should stay on screen while it fits beside the open content,
step aside only when it does not fit, and come back by itself once space
returns. (Constitution LIV, XLII, III.)

Why it happens today: opening any right panel (Changes, Logs, Preview,
Browser) on a window up to 2048px wide hides the drawer. That check reads the
window width once and saves the result as the user's own "drawer hidden"
preference. So the drawer can stay hidden across resizes and reloads even
after the panel is closed.

## User Stories
- As a user on a laptop or a regular monitor, I want the workspaces list to
  stay visible while there is room for it, so that I can switch workspaces
  without hovering or pressing shortcuts.
- As a user who resizes the browser window, I want the drawer to hide when
  the window gets too narrow for the chat and the open panels, and to return
  when I widen it again, without clicking anything.
- As a user who has deliberately hidden the drawer, I want it to stay hidden
  until I show it again, so that automatic behaviour never overrides my choice.
- As a user whose drawer was hidden for lack of space, I want the existing
  hover reopen tag and the Show Left Sidebar toggle to still bring it up.

## Functional Requirements
- FR-1: On desktop, the drawer shows whenever the user has not hidden it
  **and** the workspace area is wide enough to hold the drawer, the chat at
  its minimum usable width, any open right panel at its minimum usable
  width, and the right sidebar when that is shown.
- FR-2: When that space is not available, the drawer hides automatically.
  Only the content actually on screen counts toward the required width.
- FR-3: The decision updates live when the window is resized, a right panel
  opens or closes, the chat panel is hidden or shown, the right sidebar is
  toggled, or the Settings drawer opens or closes.
- FR-4: Hiding the drawer automatically never changes the saved
  "left sidebar visible" preference. Opening or closing a right panel does
  not hide the drawer by itself.
- FR-5: While the drawer is auto-hidden, the hover reopen tag and preview
  work as they do today when the user has hidden it.
- FR-6: The Show/Hide Left Sidebar toggle (navbar, command bar, `V S`) acts on
  what is on screen, and its label and active state match it:
  - a visible drawer is hidden, and that is saved;
  - a drawer the user hid is shown, and that is saved;
  - a drawer hidden for lack of space is shown anyway until the available
    space changes again.
- FR-7: Closing the right panel still restores the drawer, as it does today.
- FR-8: The mobile layout is unchanged.

## Out of Scope
- Resizing the drawer or changing its width.
- Auto-hiding the right sidebar or the chat.
- The kanban view, mobile tabs, and the look of the hover preview.

## Acceptance Criteria
- [ ] With only the chat open and the right sidebar shown, the drawer is
      visible in a ~1300px window (the recording's size) and stays visible
      while the window is resized within that range.
- [ ] With Changes open and the right sidebar shown, the drawer is visible
      on a 1920px screen. On a ~1440px screen it hides, and it reappears when
      the right sidebar or the Changes panel is closed, or the window is
      widened.
- [ ] Opening or closing a right panel does not change the saved preference.
- [ ] When the drawer is auto-hidden, the hover tag opens the preview and
      `V S` shows the drawer. The toggle label reads "Show Left Sidebar"
      while the drawer is hidden.
- [ ] Automated tests cover the fit rule and the toggle states.
- [ ] `pnpm run check`, `pnpm run lint` and `pnpm run format` pass.

## Clarifications
See `clarifications.md`.
- Minimum widths: chat 480px, right panel 400px, and the drawer and right
  sidebar 300px each. With only the chat and the right sidebar, the drawer
  needs 1080px of workspace area. Adding a right panel raises that to 1480px.
  So a 1920px screen keeps the drawer with Changes open, and a 1440px laptop
  does not unless the right sidebar is closed.
- An old saved "hidden" preference is treated as a real user choice (it
  cannot be told apart). Closing a right panel still restores the drawer
  (FR-7), and so does one toggle. No migration.
- The override lasts until the space situation flips (the drawer starts to
  fit, or is re-evaluated as fitting) or the user hides the drawer. It is not
  saved, so a reload drops it.

## Open Questions
None.
