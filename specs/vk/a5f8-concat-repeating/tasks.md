# Tasks: Authoritative Execution Status Reconciliation

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run together within their layer.

## Phase 1: Reproduce and lock contracts

- [x] T001 Add a frontend reconnect regression that retains a stale running
  snapshot during disconnect and replaces it with a terminal snapshot on the
  next connection in
  `packages/web-core/src/shared/hooks/useJsonPatchWsStream.reconnect.test.tsx`.
- [x] T002 Add focused event-stream coverage for an execution update published
  during snapshot initialization, plus an active-running control, in
  `crates/services/src/services/events/streams.rs` (may initially fail; depends
  on the existing stream contract only).

## Phase 2: Close the authoritative handoff gap

- [x] T003 Refactor execution-process session stream initialization to acquire
  its broadcast receiver before the database snapshot and consume that receiver
  after snapshot/Ready in
  `crates/services/src/services/events/streams.rs` (depends on T002).
- [x] T004 Verify exact-running derivation and active cancellation remain intact;
  add a focused derivation test only if current coverage is insufficient in
  `packages/web-core/src/shared/hooks/useExecutionProcesses.ts` and the nearest
  existing test file (depends on T001, T003).

## Phase 3: Lifecycle and regression verification

- [x] T005 [P] Run focused frontend tests for
  `packages/web-core/src/shared/hooks/useJsonPatchWsStream.reconnect.test.tsx`
  and any execution-process derivation test (depends on T004).
- [x] T006 [P] Run focused Rust event-stream and orphan/shutdown reconciliation
  tests covering `crates/services/src/services/events/streams.rs`,
  `crates/services/src/services/container.rs`, and
  `crates/local-deployment/src/container.rs` (depends on T003).
- [x] T007 Run `pnpm run format`, generated-type checks as applicable,
  repository checks, and lint for changed packages (depends on T005, T006).

## Phase 4: Review and handoff

- [x] T008 Run independent Codex diff review, address confirmed findings,
  re-run affected checks, and repeat until no significant findings remain
  (depends on T007).
- [x] T009 Update `docs/knowledge-base/` with the reusable subscribe-before-
  snapshot stream handoff rule, tag it `vk/3488-fix-stale-execut`, refresh
  `docs/knowledge-base/INDEX.md`, and commit the knowledge base (depends on T008).
- [ ] T010 Commit remaining implementation and documentation intentionally,
  open a pull request against the recorded base branch, pass required checks,
  and merge it (depends on T009).
