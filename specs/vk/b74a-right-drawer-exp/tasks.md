# Tasks: Right Drawer Expand to Available Space

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their layer. This task will be executed by one
agent as required by the workspace instructions.

## Phase 1: Shared layout contract

- [x] T001 Add the opt-in `fillAvailableSpace` prop and expansion-derived root
      sizing to
      `packages/ui/src/components/CollapsibleSectionHeader.tsx`.

## Phase 2: Feature integration and coverage

- [x] T002 Wire the full-height flex stack, remove the artificial maximum-height
      wrapper, and opt sections into flexible sizing in
      `packages/web-core/src/pages/workspaces/RightSidebar.tsx` (depends on
      T001).
- [x] T003 [P] Add rendered-DOM coverage for opt-in expanded/collapsed sizing
      and default compatibility in
      `packages/web-core/src/pages/workspaces/CollapsibleSectionHeader.test.tsx`
      (depends on T001).

## Phase 3: Verification and review

- [x] T004 Run repository formatting, the focused Vitest test, UI/web-core type
      checks, and relevant lint (depends on T002, T003; no file intended).
- [ ] T005 Run independent Codex review of the implementation diff, address all
      confirmed significant findings, and repeat checks as needed (depends on
      T004; findings may change the files from T001-T003).

## Phase 4: Knowledge capture

- [ ] T006 Distill the reusable flex-panel layout rule into
      `wiki/right-drawer-flexible-sections.md` and refresh `wiki/INDEX.md`
      (depends on T005).
- [ ] T007 Commit the knowledge-base changes before task handoff (depends on
      T006; changes Git history only).

## Dependency graph

`T001 -> (T002 || T003) -> T004 -> T005 -> T006 -> T007`
