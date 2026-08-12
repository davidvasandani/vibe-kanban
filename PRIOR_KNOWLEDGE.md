# Prior knowledge: mobile workspace right drawer

Relevant project knowledge is not empty.

## Matches

### `wiki/flexible-collapsible-panel-stacks.md`

- `RightSidebar` is intentionally shared between the desktop workspace drawer
  and the mobile Git tab.
- Mobile must reuse that composition; desktop-only chrome must be enabled by
  the desktop mount instead of being added unconditionally to the shared
  component.
- The drawer owns a bounded `min-h-0` flex chain and an outer vertical overflow
  fallback. Access changes must not disturb those sizing and scrolling rules.

### `wiki/workspace-context-bar-responsive-visibility.md`

- The responsive workspace layout is selected by `useIsMobile()` at the
  project's 767px breakpoint; physical-device detection is not authoritative
  for layout chrome.
- Mobile already exposes workspace-adjacent destinations as navbar tabs.
  Therefore the existing tab architecture is the appropriate access point for
  the right drawer, rather than duplicating desktop controls or adding a new
  floating context bar.
- Layout-specific policy belongs in the workspace/container composition while
  shared presentational components should remain broadly reusable.

### `wiki/workspace-navbar-breadcrumbs.md`

- `NavbarContainer` is the stateful boundary between workspace context and the
  shared `Navbar` presentation.
- Async workspace identity can be temporarily unavailable, so a drawer affordance
  should not assume selected-workspace data is always ready.

## Consequences for this task

1. Keep the existing `git` mobile-tab state identifier so persisted preferences
   and `WorkspacesLayout` routing remain compatible.
2. Make the tab visibly and accessibly describe the right sidebar rather than
   introducing a second drawer instance or a new visibility store.
3. Preserve `RightSidebar`'s flex/overflow composition and desktop-only chrome
   contract.
4. Test at the shared navbar boundary for control semantics and at the workspace
   layout boundary only where necessary to prove the existing drawer mount is
   selected.
5. Use responsive layout state, not user-agent detection, for any conditional
   behavior.
