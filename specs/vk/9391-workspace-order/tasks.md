# Tasks: Stable Workspace Order During Restart

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run in parallel within their layer.

## Phase 1: Ordering Contract

- [x] T001 Create the pure workspace sort helper with timestamp fallback,
  pinning, missing-value, direction, and tie-break behavior in
  `packages/web-core/src/pages/workspaces/workspaceSort.ts`.
- [x] T002 Wire the helper into both active and archived sidebar sorting in
  `packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx`
  (depends on T001).

## Phase 2: Regression Coverage

- [x] T003 Add focused base-only, enriched, direction, pinning, invalid-value,
  and deterministic-tie tests in
  `packages/web-core/src/pages/workspaces/workspaceSort.test.ts` (depends on
  T001).

## Phase 3: Verification and Review

- [x] T004 Run the relevant workspace-sort tests and frontend type checks;
  resolve implementation regressions in the Phase 1–2 files (depends on T002,
  T003).
- [x] T005 Run `pnpm run format` and inspect the final Vibe Kanban diff (depends
  on T004).
- [x] T006 Run independent Codex CLI review, record results in
  `specs/vk/9391-workspace-order/review.md`, and address confirmed significant
  findings in their affected files until clean (depends on T005).

## Phase 4: Knowledge and Delivery

- [x] T007 Add reusable partial-projection ordering knowledge tagged
  `vk/9391-workspace-order` to
  `docs/knowledge-base/workspace-summary-ordering.md` and refresh
  `docs/knowledge-base/INDEX.md` (depends on T006).
- [ ] T008 Commit the knowledge base and completed task, open a pull request,
  pass required checks, and merge it (depends on T007).
