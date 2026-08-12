# Tasks: Compact Right Drawer Section Spacing

**Plan**: `./plan.md`

Tasks are dependency-ordered. Tasks marked **[P]** touch independent files and
may run together within a layer.

## Layer 1: Composition Contract

- [x] T001 Add explicit per-section available-height metadata and make Server
  Affinity intrinsic while preserving all other section policies in
  `packages/web-core/src/pages/workspaces/RightSidebar.tsx`.

## Layer 2: Regression Coverage

- [x] T002 Add rendered composition assertions for intrinsic Server Affinity
  and flexible content panels in
  `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx` (depends on
  T001).

## Layer 3: Verification

- [x] T003 Run focused `RightSidebar` and `CollapsibleSectionHeader` Vitest
  suites (depends on T001–T002).
- [x] T004 [P] Run `web-core` and `ui` TypeScript checks (depends on T001–T002).
- [x] T005 [P] Run relevant frontend lint checks (depends on T001–T002).
- [x] T006 Run repository formatting and `git diff --check`, then rerun affected
  checks if formatting changes code (depends on T003–T005).

## Layer 4: Independent Review

- [x] T007 Run an independent Codex CLI review of the complete task diff,
  address confirmed significant findings, and repeat Layer 3 plus review until
  clean (depends on T006).

## Layer 5: Knowledge and Delivery

- [x] T008 Update the relevant project knowledge page and
  `docs/knowledge-base/INDEX.md`, tagged with `d80e-fix-the-spacing` (depends on
  T007).
- [ ] T009 Commit the final Vibe Kanban change, push the task branch, open a pull
  request against the base branch, wait for required checks, and merge it
  (depends on T008).
