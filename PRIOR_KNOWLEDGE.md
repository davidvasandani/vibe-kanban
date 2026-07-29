# Prior Knowledge: Mobile Workspace Context Bar

**Task ID:** `vk/2792-vk-workspace-flo`

## Knowledge-base search

Searched `wiki/INDEX.md` and all topic pages for `context bar`, `floating`,
`mobile`, `workspace layout`, `responsive`, and `drag`.

No page directly documents the single-workspace floating context bar or its
mobile visibility rule. The following existing pages contain relevant,
reusable constraints.

## Relevant matches

### `wiki/workspace-carousel-view.md`

- `WorkspacesMainContainer` is prop-driven and owns the chat UI composition.
- It exposes `hideContextBar` for contexts where the route-level action state
  is unavailable. This confirms that the context bar is optional workspace
  chat chrome, not required conversation content.
- The carousel deliberately hides it because context-bar actions resolve
  through the single-workspace route's action visibility context.

**Implication for this task:** suppressing the context bar in another layout
where it is redundant is consistent with existing component design. The fix
should stay at the container/composition boundary rather than changing chat
content.

### `wiki/mobile-kanban-scrolling.md`

- Mobile behavior is selected with `useIsMobile()`.
- Mobile touch interactions are sensitive to scroll ownership and CSS
  overflow; desktop mouse interaction assumptions should not be carried into
  the mobile layout.
- The project has no touch engine in its automated task environment, so
  focused automated invariants must be paired with the documented manual
  mobile verification approach when possible.

**Implication for this task:** use the project's responsive mobile signal and
avoid adding a second touch/drag interaction to a redundant floating overlay.
Test the visibility rule directly, and preserve desktop behavior.

### `wiki/workspace-navbar-breadcrumbs.md`

- Workspace navbar state preparation belongs in `web-core`; the shared
  `packages/ui` Navbar remains presentational.

**Implication for this task:** responsive/device policy should remain in the
`web-core` context-bar container. The presentational `packages/ui`
`ContextBar` does not need a device-awareness prop or platform import.

## Distilled constraints for specification and planning

1. Treat the floating context bar as optional workspace chrome.
2. Keep layout/platform policy in `packages/web-core`, not `packages/ui`.
3. Use `useIsMobile()` as the responsive-layout source of truth.
4. Do not solve the defect by adding touch dragging to a duplicated control.
5. Preserve the desktop context bar and persisted snap behavior.
6. Add an automated test for disagreement between responsive and
   physical-device detection.
7. Plan for manual mobile verification limitations documented by the project.
