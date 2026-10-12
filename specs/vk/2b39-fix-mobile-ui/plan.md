# Implementation Plan: Fix mobile issue list UI

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context
React 18 + TypeScript, Tailwind (design tokens such as `gap-half`,
`gap-base`, `px-double`), `@hello-pangea/dnd` for list drag-and-drop.
Presentational components live in `packages/ui` (`@vibe/ui`). The container
that decides the layout is `packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`,
which already computes `isMobile = useIsMobile()` (`max-width: 767px`). Nothing
changes on the backend, in data, or in generated types.

## Architecture & Approach
The cause is in `packages/ui/src/components/IssueListRow.tsx`. It is a single
`justify-between` flex row whose right cluster (tags, relationships, assignee,
age) is `shrink-0`. On a narrow screen that cluster is wider than the row, so
the `flex-1 min-w-0` left cluster shrinks to zero, and its `shrink-0`
children (handle, id, dot) end up under the badges.

1. **`IssueListRow`**: add `isMobile?: boolean`. Move the handle/checkbox
   block, the metadata cluster (tags + relationships + "+N") and the age
   label into local JSX constants, so both branches use identical pieces.
   - `!isMobile`: the same element tree and class strings as today (FR-5).
   - `isMobile`: the root becomes `flex flex-col gap-half px-base py-half`.
     - Line 1 is `flex items-center gap-base min-w-0`: handle, priority, id
       (`shrink-0`), dot, title (`flex-1 min-w-0 truncate`), age
       (`shrink-0`, no fixed `w-5`, so `52d` cannot clip).
     - Line 2 renders only when the issue has tags, relationships or
       assignees: `flex items-center gap-half min-w-0 overflow-hidden pl-*`.
       The `pl-*` lines it up with the id column (handle `w-4` + gap). Tags
       come first, then relationships, then the assignee with `ml-auto
       shrink-0`. Badges keep `whitespace-nowrap` and are clipped at the
       row edge (FR-3).
   - The `Draggable`, `role="button"`, click and keyboard handlers, and the
     checkbox logic do not change (FR-4).
2. **`IssueListSection`** / **`IssueListView`**: add `isMobile?: boolean`
   and pass it down. `IssueListView`'s root scroller gets
   `overflow-x-hidden`, per the wiki's Gotcha 1 and constitution XXXVI.
3. **`KanbanContainer`**: pass `isMobile` to `IssueListView`. The list
   wrapper uses `px-base` on mobile (matching the header's mobile padding)
   and `overflow-x-hidden`.
4. **`KanbanFilterBar`**: when `isMobile && hasActiveFilters`, render an
   icon-only button (`XIcon`, styled like the existing mobile filter/search
   buttons, `aria-label`/`title` = `t('kanban.clearFilters')`). Desktop
   keeps the `PrimaryButton` (FR-6). No new i18n keys.
5. **Mockups** (FR-7): `mockups/mobile-issue-list.html` is a static,
   self-contained page with four 390px phone frames built from the
   screenshot's data, plus a PNG rendered by agent-browser.

## Data Model
Not applicable. No data changes (no `data-model.md`).

## Contracts
`./contracts/ui-props.md`: the optional prop additions.

## Research Notes
See `./research.md`. No new dependencies.

## Constitution Check
- **IV (shared-component boundaries)**: the layout policy (`useIsMobile`)
  stays in web-core, and `ui` only takes an optional prop. Both `local-web`
  and `remote-web` use `KanbanContainer`, so both get the fix.
- **III (small steps)**: no new components. Desktop markup is untouched.
- **XXXVI (one scroll authority)**: the list scrollers clip on the x axis.
- **XLVIII (lint)**: no ESLint directive comments.
- **LIII (dense rows by priority)**: id, status and title come first, then
  age, then the metadata line, which clips. Nothing overlaps.
- **II (test the contract)**: `packages/ui` has no component test harness
  (no vitest/RTL in `packages/ui/package.json`). Validation is the rendered
  390px check plus `check`/`lint`. Recorded in research R4.
No deviations.

## Risks & Dependencies
- Touch devices fire `:hover` on tap, which can swap the drag handle for the
  checkbox. That behavior predates this change and is out of scope; it is
  noted in research.
- No touch engine is available in the task environment. The visual check is
  a desktop browser at a 390px viewport. Phone checks follow
  `mobile-testing.md`.
