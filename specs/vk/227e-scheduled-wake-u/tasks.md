# Tasks (/speckit.tasks)

Plan: ./plan.md

- [x] T001 Install dependencies and add failing behavioral regression tests in packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.test.tsx.
- [x] T002 Repair terminal-first reconciliation and asynchronous scope/membership guards in packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts (depends T001).
- [ ] T003 Run focused regression suite, frontend checks/lint and repository format; capture validation in specs/vk/227e-scheduled-wake-u/validation.md (depends T002).
- [x] T004 Run independent Codex CLI diff review; address significant findings and re-verify (depends T003).
- [x] T005 Update wiki/durable-session-wakeups.md and wiki/INDEX.md; commit reusable knowledge (depends T004).
- [ ] T006 Open and merge PR after required checks (depends T005).

No independent implementation layers require [P]; tests and implementation touch the same behavioral boundary and proceed sequentially.
