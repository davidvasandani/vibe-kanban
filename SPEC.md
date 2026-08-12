# Mobile access to the workspace right drawer

Task: `a12b9b02-6250-42e9-b5b0-220ea5fca2af`

## Problem

The desktop workspace layout has a right sidebar and a dedicated action for
showing or hiding it. In the mobile layout the same `RightSidebar` content is
mounted under the `git` mobile tab, represented by a git-fork glyph in the
top tab strip. That affordance does not identify itself as the right drawer,
and the desktop `ToggleRightSidebar` action is not rendered in the mobile
navbar. A mobile user can therefore reasonably conclude that the drawer is
missing.

## Objective

Make the workspace right drawer discoverable and directly accessible on mobile
without changing the existing desktop panel behavior or the contents of the
drawer.

## Required behavior

- A mobile workspace screen exposes a clearly identifiable control for the
  right drawer in the top navigation.
- Activating the control shows the existing `RightSidebar` content for the
  selected workspace.
- The control communicates its purpose through an accessible name and the
  established right-sidebar iconography.
- The active state is visible while the right drawer is selected.
- The control remains usable at narrow phone widths and participates in the
  existing horizontally scrollable mobile tab strip.
- Create mode and workspace-less states do not render unusable drawer content.
- Desktop `ToggleRightSidebar`, persistence, and resizable-panel behavior stay
  unchanged.
- Existing mobile tabs (workspaces, chat, changes, logs, preview, and browser)
  keep their behavior and state preservation.

## Technical direction

Use the existing mobile-tab architecture rather than introducing a second
overlay or a competing visibility state. Give the existing mobile right-drawer
tab explicit sidebar semantics in the shared navbar configuration, while
retaining the current `git` tab identifier to avoid a preference migration.
Keep `WorkspacesLayout` as the owner of rendering `RightSidebar` for that tab.

## Scope

This is a Vibe Kanban frontend change in `packages/ui` and/or
`packages/web-core`. No other service or homelab deployment configuration is in
scope.

## Verification

- Component coverage proves the mobile navbar renders a right-drawer control
  with an accessible name, selects it, and exposes its active state.
- Workspace layout coverage proves the selected mobile tab displays the
  existing `RightSidebar` and other tab content remains hidden/preserved.
- Relevant frontend tests, type checking, linting, and formatting pass.
- A narrow mobile viewport is inspected to confirm the control remains
  reachable and visually understandable.

## Non-goals

- Redesigning the contents of `RightSidebar`.
- Changing desktop sidebar sizing or persistence.
- Adding a new backend API or persisted preference field.
- Updating any service other than Vibe Kanban.
