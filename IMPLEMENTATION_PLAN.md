# Implementation plan: Fix mobile issue list UI (`vk/2b39-fix-mobile-ui`)

See `SPEC.md`. All changes are frontend; no Rust/type generation.

1. **Mockups** — `specs/vk/2b39-fix-mobile-ui/mockups/mobile-issue-list.html`:
   static 390px-wide frames reproducing the screenshot data for
   (0) current/broken, (A) two-line rows *(recommended)*, (B) single line with
   collapsed metadata counters, (C) stacked cards. Render to PNG with
   agent-browser (`mobile-issue-list.png`).
2. **`IssueListRow`** (`packages/ui/src/components/IssueListRow.tsx`)
   - Add `isMobile?: boolean`.
   - Extract the handle/checkbox block and metadata cluster into local
     render helpers so desktop and mobile share them.
   - Desktop branch: identical classes/markup to today.
   - Mobile branch: `flex-col` row; line 1 = handle, priority, id, dot,
     title (`flex-1 min-w-0 truncate`), age (`shrink-0`, no fixed `w-5`);
     line 2 only if tags/relationships/assignees exist, indented to align
     with the id, `overflow-hidden`, assignee `ml-auto`.
3. **`IssueListSection` / `IssueListView`** — accept and forward `isMobile`.
   `IssueListView` adds `overflow-x-hidden` to its scroller.
4. **`KanbanContainer`** — pass `isMobile` to `IssueListView`; list wrapper
   uses `px-base overflow-x-hidden` on mobile (keep `px-double` on desktop).
5. **`KanbanFilterBar`** — on mobile render Clear filters as an icon-only
   `XIcon` button with `aria-label`/`title` = `kanban.clearFilters`.
6. Verify: `pnpm run format`, `pnpm run check`, `pnpm run lint`; render a
   390px harness of the real row classes (or the mockup) for visual check.
7. Codex review → fix → wiki page `mobile-issue-list-rows.md` + INDEX →
   PR → merge.
