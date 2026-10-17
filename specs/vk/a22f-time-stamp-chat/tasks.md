# Tasks: Timestamp the Workspace Chat Log

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` denotes tasks in the same layer that touch
independent files.

## Phase 1: Timestamp contract

- [x] T001 Add pure timestamp selection/formatting and semantic presentation,
  with valid/current-day/cross-day/invalid/aggregate coverage, in
  `packages/web-core/src/features/workspace-chat/ui/ConversationTimestamp.tsx`
  and
  `packages/web-core/src/features/workspace-chat/ui/ConversationTimestamp.test.tsx`.

## Phase 2: Source preservation and rendering

- [x] T002 Preserve execution `created_at` on derived user and script
  normalized entries and add derivation regression coverage in
  `packages/web-core/src/features/workspace-chat/model/deriveConversationEntries.ts`
  and
  `packages/web-core/src/features/workspace-chat/model/deriveConversationEntries.test.ts`
  (depends on T001).
- [x] T003 Integrate timestamp presentation at the shared conversation-row
  wrapper and add representative rendered-DOM coverage in
  `packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry.tsx`
  and
  `packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry.test.tsx`
  (depends on T001).

## Phase 3: Validation

- [x] T004 Run focused workspace-chat tests, frontend type checks, lint,
  repository formatting, and `git diff --check` (depends on T002 and T003; no
  files intentionally changed beyond formatter output).
- [x] T005 Record acceptance and verification evidence in
  `specs/vk/a22f-time-stamp-chat/verification.md` (depends on T004).

Independent review, reusable-knowledge distillation, commits, pull-request
creation, CI monitoring, and merge remain pipeline stages 11–13 and are not
duplicated in `/speckit.implement`.
