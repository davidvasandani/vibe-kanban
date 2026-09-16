# Tasks: Show Sent Chat Messages Without Refreshing

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the files it
changes.

## Phase 1: Reconciliation contract

- [x] T001 Extend the execution-process context with the scoped reconciliation
      action in
      `packages/web-core/src/shared/hooks/useExecutionProcessesContext.ts`.
- [x] T002 Implement ID-keyed, session-scoped response/stream reconciliation in
      `packages/web-core/src/shared/providers/ExecutionProcessesProvider.tsx`
      (depends on T001).
- [x] T003 Add response-first, stream-first, live-supersession, mismatch, and
      session-reset provider tests in
      `packages/web-core/src/shared/providers/ExecutionProcessesProvider.test.tsx`
      (depends on T002).

## Phase 2: Send-path integration

- [x] T004 [P] Add the success-only accepted-process callback to
      `packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend.ts`
      (depends on T001).
- [x] T005 [P] Add successful, failed, and new-session callback tests in
      `packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend.test.tsx`
      (depends on T004).
- [x] T006 Wire the existing-session chat container to the reconciliation action
      in
      `packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
      (depends on T002, T004).

## Phase 3: Validation and documentation

- [x] T007 Run focused provider/send/conversation Vitest tests, formatting,
      frontend type checks, lint, and relevant repository checks; record results
      in `specs/vk/8a08-frontend-not-ref/verification.md` (depends on T003,
      T005, T006).
- [x] T008 [P] Reconcile shipped behavior into `SPEC.md`,
      `IMPLEMENTATION_PLAN.md`, and
      `specs/vk/8a08-frontend-not-ref/{spec.md,plan.md,tasks.md}` (depends on
      T006).
- [x] T009 Run independent Codex review, resolve all confirmed significant
      findings in the task files, rerun affected verification, and record the
      clean result in `specs/vk/8a08-frontend-not-ref/review.md` (depends on
      T007, T008).
- [x] T010 Add the reusable request/stream reconciliation pattern with tag
      `vk/8a08-frontend-not-ref` to
      `docs/knowledge-base/authoritative-snapshot-stream-handoffs.md` and
      refresh `docs/knowledge-base/INDEX.md` (depends on T009).
- [ ] T011 Commit task and knowledge-base changes, push the branch, open a pull
      request against `main`, verify checks, and merge it (depends on T010).
