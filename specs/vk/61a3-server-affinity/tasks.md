# Tasks: Server Affinity Sidebar Polish

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the files it changes.

## Phase 1: Baseline and test seams

- [x] T001 Reconcile the branch with `origin/main`, preserving task documents
  in `SPEC.md`, `PRIOR_KNOWLEDGE.md`, `IMPLEMENTATION_PLAN.md`,
  `.specify/memory/constitution.md`, and `specs/vk/61a3-server-affinity/*`
- [x] T002 Inspect the reconciled sidebar tests and establish a focused affinity
  label test seam in
  `packages/web-core/src/pages/workspaces/RightSidebar.tsx` and/or a colocated
  `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx` (depends on
  T001)

## Phase 2: UI implementation

- [x] T003 Implement the summary-backed, bounded collapsed-header server label
  in `packages/web-core/src/pages/workspaces/RightSidebar.tsx` (depends on T002)
- [x] T004 [P] Implement the compact responsive two-column body layout in
  `packages/web-core/src/pages/workspaces/ServerAffinitySectionContainer.tsx`
  (depends on T001)
- [x] T005 Add or finish focused regression tests for hostname precedence,
  absent summary behavior, and disclosure-safe truncation in the selected
  colocated test file (depends on T003; can run in the same layer as T004)

## Phase 3: Validation and evidence

- [x] T006 Run formatting over the changed frontend files and task documents;
  accept only scoped formatting changes (depends on T003–T005)
- [x] T007 [P] Run focused Vitest coverage for the changed workspace sidebar
  test file(s) (depends on T006)
- [x] T008 [P] Run the frontend TypeScript/check command for the affected
  package(s) (depends on T006)
- [x] T009 [P] Run the frontend lint command for the affected package(s)
  (depends on T006)
- [x] T010 Record commands and results in
  `specs/vk/61a3-server-affinity/validation.md` (depends on T007–T009)

## Phase 4: Independent review

- [ ] T011 Run the independent Codex diff review and record findings in
  `specs/vk/61a3-server-affinity/review.md` (depends on T010)
- [ ] T012 Address every confirmed significant review finding in the affected
  implementation/tests and rerun T007–T009 until the independent review is
  clean (depends on T011)

## Phase 5: Knowledge distillation

- [ ] T013 Distill reusable collapsed-header/responsive-flex guidance, if any,
  into the appropriate page under `docs/knowledge-base/` with task tag `61a3`,
  and refresh `docs/knowledge-base/INDEX.md` (depends on T012)
- [ ] T014 Commit the knowledge-base update separately, or record “no new
  knowledge to record” in `specs/vk/61a3-server-affinity/validation.md` if no
  reusable knowledge emerged (depends on T013)
