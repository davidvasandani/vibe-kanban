# Tasks: Lazy-load workspace chat history requirements

**Plan**: `./plan.md`

This task is an investigation and requirements deliverable. Product-code tasks
below stop at implementation-ready design; implementing the proposed transport
is a follow-up feature, not an unrequested mutation in this task.

Tasks are dependency-ordered. `[P]` tasks in the same layer touch independent
files and may be completed together.

## Layer 1 — Evidence and scope

- [x] T001 Trace current frontend history loading, batching, retained state, and
  scroll behavior in
  `packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts`
  and
  `packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx`.
- [x] T002 [P] Trace normalized history routing, storage, reconstruction, and
  cancellation in `crates/server/src/routes/execution_processes.rs`,
  `crates/services/src/services/container.rs`, and
  `crates/services/src/services/execution_process.rs`.
- [x] T003 [P] Search and distill relevant reusable knowledge into
  `PRIOR_KNOWLEDGE.md`. Depends on T001/T002 only for final implications.

## Layer 2 — Requirements and decisions

- [x] T004 Write the repo-root technical requirements and acceptance criteria in
  `SPEC.md`. Depends on T001/T002.
- [x] T005 Establish the bounded-history constitutional invariant in
  `.specify/memory/constitution.md`. Depends on T001/T002.
- [x] T006 Write and clarify user-facing functional requirements in
  `specs/vk/65ab-lazy-load-vk-wor/spec.md` and
  `specs/vk/65ab-lazy-load-vk-wor/clarifications.md`. Depends on T003/T004/T005.

## Layer 3 — Implementation-ready design

- [x] T007 Document current constraints and selected architecture in
  `specs/vk/65ab-lazy-load-vk-wor/research.md`. Depends on T006.
- [x] T008 [P] Define durable normalized state, page/cursor, and frontend state
  in `specs/vk/65ab-lazy-load-vk-wor/data-model.md`. Depends on T006.
- [x] T009 [P] Define the bounded page and snapshot/live contract in
  `specs/vk/65ab-lazy-load-vk-wor/contracts/history-api.md`. Depends on T006.
- [x] T010 Write the grounded future implementation plan in `IMPLEMENTATION_PLAN.md`
  and `specs/vk/65ab-lazy-load-vk-wor/plan.md`. Depends on T007/T008/T009.

## Layer 4 — Validation and handoff

- [x] T011 Cross-check specification, plan, data model, contract, and tasks
  against `.specify/memory/constitution.md`; record findings in
  `specs/vk/65ab-lazy-load-vk-wor/analyze.md`. Depends on T010.
- [x] T012 Verify all cited source paths/symbols and confirm no files outside
  `vibe-kanban/` were changed. Depends on T011.
- [x] T013 Run independent Codex review of the documentation diff, resolve
  significant findings, and repeat until clean. Depends on T012.
- [x] T014 Distill reusable bounded-history knowledge into
  `docs/knowledge-base/`, tag task `65ab-lazy-load-vk-wor`, refresh
  `docs/knowledge-base/INDEX.md`, and commit the knowledge-base update. Depends
  on T013.

## Follow-up product implementation (not executed by this requirements task)

- [ ] F001 Persist revisioned materialized normalized entries in the services
  layer, including legacy cancellable build and correctness tests.
- [ ] F002 Add the authorized session history page API, cursor validation,
  limits, ordering, and route/service tests.
- [ ] F003 Add snapshot-watermarked live resume and race tests.
- [ ] F004 Replace frontend background preload with recent-page plus
  single-flight `loadEarlier` state and stale-scope rejection.
- [ ] F005 Add top sentinel/control, accessible errors/retry, and semantic
  scroll-anchor preservation with frontend tests.
- [ ] F006 Regenerate contracts as needed and run full format/check/test/review.

## Parallel execution notes

- T001 and T002 are independent traces; T003 can begin from indexed knowledge.
- T008 and T009 use independent files after clarification.
- Follow-up F004 can start against the approved API contract while backend
  F001/F002 are developed, but integration waits for F002/F003.
