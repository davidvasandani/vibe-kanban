# Tasks: Fix mobile issue list UI

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Mockups & leaf components
- [x] T001 [P] Build mockups 0/A/B/C (390px frames, screenshot data) in `specs/vk/2b39-fix-mobile-ui/mockups/mobile-issue-list.html`
- [x] T002 [P] Add `isMobile` two-line layout to `packages/ui/src/components/IssueListRow.tsx` (desktop branch unchanged)
- [x] T003 [P] Icon-only mobile clear-filters button in `packages/ui/src/components/KanbanFilterBar.tsx`

## Phase 2: Wiring
- [x] T004 Forward `isMobile` in `packages/ui/src/components/IssueListSection.tsx` (depends on T002)
- [x] T005 Forward `isMobile` + `overflow-x-hidden` in `packages/ui/src/components/IssueListView.tsx` (depends on T004)
- [x] T006 Pass `isMobile`, mobile `px-base` + `overflow-x-hidden` list wrapper in `packages/web-core/src/features/kanban/ui/KanbanContainer.tsx` (depends on T005)

## Phase 3: Validation
- [x] T007 [P] Render mockups to `specs/vk/2b39-fix-mobile-ui/mockups/mobile-issue-list.png` (depends on T001)
- [x] T008 [P] Run `pnpm run format`, `pnpm run check`, `pnpm run lint` (depends on T003, T006)
- [x] T009 Render the real `IssueListRow` + `KanbanFilterBar` at 390px via SSR + the real Tailwind config (harness under `/tmp`, not committed); confirm no overlap/clip, `role="button"` + drag handle present; save `mockups/shipped-mobile-390.png` (depends on T006)
