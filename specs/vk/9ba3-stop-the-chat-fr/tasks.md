# Tasks: Stable Streaming Chat Viewport

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` denotes tasks in the same layer that touch
independent files.

## Phase 1: Regression contract

- [x] T001 Add the pure edge-triggered plan-reveal transition and unit tests for
  first/repeated/distinct plan identities and conversation-scope reset in
  `packages/web-core/src/features/workspace-chat/model/plan-reveal-transition.ts`
  and
  `packages/web-core/src/features/workspace-chat/model/plan-reveal-transition.test.ts`.

## Phase 2: Integration

- [x] T002 Integrate per-conversation revealed-plan identity into
  `packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts`,
  preserving the incoming update type for repeated snapshots (depends on T001).

## Phase 3: Validation

- [x] T003 Run the focused conversation scroll tests, then frontend type checks,
  ESLint, formatting, and `git diff --check` (depends on T002; no files
  intentionally changed beyond formatter output).
- [x] T004 Record implementation verification and acceptance evidence in
  `specs/vk/9ba3-stop-the-chat-fr/verification.md` (depends on T003).

Independent review, reusable-knowledge distillation, commits, pull-request
creation, CI monitoring, and merge remain pipeline stages 11–13 and are not
duplicated in `/speckit.implement`.
