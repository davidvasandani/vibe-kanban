# Tasks: Commits Behind in the Git Header

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may be performed together within their layer. Stages 11–13 (independent review,
knowledge-base distillation, and PR merge) remain pipeline-owned after
`/speckit.implement` and are not reordered into this list.

## Phase 1: Header view model and presentation

- [x] T001 Implement the ID-joined, positive-only behind-status derivation and
  bounded header presentation in
  `packages/web-core/src/pages/workspaces/GitBehindHeader.tsx`.
- [x] T002 Wire `GitBehindHeader` into the Git section's existing `headerExtra`
  seam without changing its sizing/expansion contract in
  `packages/web-core/src/pages/workspaces/RightSidebar.tsx` (depends on T001).

## Phase 2: Regression coverage

- [x] T003 [P] Add focused derivation and presentation tests for unavailable,
  zero, single-repository, multi-repository, singular/plural, stable ordering,
  and ID-based matching behavior in
  `packages/web-core/src/pages/workspaces/GitBehindHeader.test.tsx` (depends on
  T001).
- [x] T004 [P] Extend rendered sidebar coverage to prove the indicator is in the
  Git header and survives collapsing the Git body in
  `packages/web-core/src/pages/workspaces/RightSidebar.test.tsx` (depends on
  T002).

## Phase 3: Verification

- [x] T005 Install locked dependencies if required and run the focused Vitest
  files for `GitBehindHeader` and `RightSidebar` (depends on T003, T004).
- [x] T006 Run repository-required formatting, frontend type checks, and lint;
  resolve task-caused failures and record verification in
  `specs/vk/a35b-commits-behind-m/verification.md` (depends on T005).
