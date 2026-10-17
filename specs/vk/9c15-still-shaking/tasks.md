# Tasks: Stable Earlier-History Loading Geometry

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` denotes tasks in the same layer that touch
independent files.

## Phase 1: Regression Contract

- [x] T001 Add rendered-DOM coverage for idle, loading, and retry presentations
  and invariant geometry in
  `packages/web-core/src/features/workspace-chat/ui/EarlierHistoryControl.test.tsx`.
- [x] T002 Demonstrate that T001 fails before the production component/fix exists
  (depends on T001; no intentional file changes).

## Phase 2: Implementation

- [x] T003 Implement the geometry-stable presentation in
  `packages/web-core/src/features/workspace-chat/ui/EarlierHistoryControl.tsx`
  (depends on T002).
- [x] T004 Replace the inline variable-height history UI with the component in
  `packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx`
  while preserving pagination and anchoring (depends on T003).

## Phase 3: Validation

- [x] T005 [P] Run focused earlier-history, paging, plan-reveal, and scroll tests
  (depends on T004; no intentional file changes).
- [x] T006 [P] Run web-core and frontend type checks/lint (depends on T004; no
  intentional file changes).
- [x] T007 Run repository formatting and `git diff --check` after T005 and T006
  (formatter may update touched TypeScript files).
- [x] T008 Record verification and acceptance evidence in
  `specs/vk/9c15-still-shaking/verification.md` (depends on T007).

Independent review, reusable-knowledge distillation, commits, pull-request
creation, CI monitoring, and merge remain pipeline stages 11–13 and are not
duplicated in `/speckit.implement`.
