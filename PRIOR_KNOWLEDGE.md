# Prior knowledge — vk/2b39-fix-mobile-ui (Fix mobile issue list UI)

Distilled from `vibe-kanban/wiki/` (read-only). Relevant pages:

## mobile-kanban-scrolling.md
- Mobile board uses nested single-axis scrollers. `overflow-y-auto` alone
  computes `overflow-x` to `auto` → any vertical scroller on mobile must also
  carry `overflow-x-hidden` (Gotcha 1). Applies to the list view scroller
  (`KanbanContainer` list wrapper + `IssueListView` root), which today only
  sets `overflow-y-auto` while rows overflow horizontally.
- Don't use `touch-action` to fight stolen gestures; constrain `overflow-*`.
- No touch engine in the task env; `mobile-testing.md` documents phone testing.
  Manual checks: long-press drag handle still drags; desktop unchanged.

## workspace-context-bar-responsive-visibility.md
- Layout decisions use `useIsMobile()` (`max-width: 767px`), not UA sniffing.
- Keep presentational `packages/ui` components layout-agnostic; policy lives in
  `web-core` and is passed down (here: an optional `isMobile` prop, same as
  `KanbanFilterBar`, `KanbanCardContent`, `KanbanBoard`).

## kanban-items-state-and-activity-grouping.md
- `items` feeds both the board and `IssueListView`; list-view drags persist
  `sort_order` from render order, so the row's `Draggable` index/order must not
  change — layout-only changes inside the row are safe.

## frontend-linting.md
- `pnpm run lint` covers `ui`; no ESLint directive comments allowed. Unused
  i18n keys are checked — reuse existing `kanban.clearFilters`.

No prior page covers the list-view row layout itself; this task should add one.
