# Tasks: Resource-Aware Chat Loading

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run together within their dependency layer.

## Phase 1: Lock the coordination contract

- [x] T001 Add deterministic execution-keyed single-flight registry tests for
  one leader, same-key waiters, different-key independence, cancellation,
  retry, and weak-cell reclamation in
  `crates/services/src/services/container.rs` (depends on existing code only).
- [x] T002 [P] Verify the existing normalized sidecar tests cover valid replay,
  incomplete file rejection, and atomic completion assumptions in
  `crates/services/src/services/normalized_log_cache.rs` (depends on existing
  code only).

## Phase 2: Implement single-flight historical materialization

- [x] T003 Implement the weakly retained execution-ID coordination registry and
  ownership guard in `crates/services/src/services/container.rs` (depends on
  T001).
- [x] T004 Refactor normalized-cache replay construction so optimistic and
  post-wait cache checks share one validation/replay path in
  `crates/services/src/services/container.rs` (depends on T002, T003).
- [x] T005 Acquire per-execution ownership after an optimistic cache miss,
  recheck the sidecar before global capacity, and retain ownership through
  completed/canceled stream lifetime in
  `crates/services/src/services/container.rs` (depends on T004).
- [x] T006 Add structured cache-hit, join-wait, ownership, capacity-wait,
  normalization, completion, failure, cancellation, and truncation diagnostics
  without transcript content in `crates/services/src/services/container.rs`
  (depends on T005).

## Phase 3: Integration and resource verification

- [x] T007 Add service-level regression coverage proving two same-execution
  readers cause one reconstruction/materialization outcome, a later hit bypasses
  capacity, and failure/drop remains retryable in
  `crates/services/src/services/container.rs` and existing test fixtures
  (depends on T005, T006).
- [x] T008 [P] Verify the WebSocket acquisition/drop path still cancels waiting
  or leader work and preserves clean completion in
  `crates/server/src/routes/execution_processes.rs`; add a focused test only if
  the service-level seam cannot cover the contract (depends on T005).
- [x] T009 [P] Establish cold-leader, concurrent-waiter, and warm-cache
  measurement evidence using a representative existing long-log fixture when
  available and structured runtime timing otherwise, documenting the available
  fixture boundary and measurement path in
  `specs/vk/6df4-loading-chat-pin/verification.md` (depends on T006).
- [x] T010 Run targeted `crates/services`, `crates/server`, and normalized-cache
  tests and address regressions in the affected Vibe Kanban files (depends on
  T007, T008, T009).

## Phase 4: Repository verification

- [x] T011 Run `pnpm install --frozen-lockfile` if the fresh worktree needs it,
  then `pnpm run format`, affected checks/tests, `pnpm run check`, and
  `pnpm run lint`; record exact results in
  `specs/vk/6df4-loading-chat-pin/verification.md` (depends on T010).
- [x] T012 Cross-check final implementation and evidence against `SPEC.md`,
  `PRIOR_KNOWLEDGE.md`, `IMPLEMENTATION_PLAN.md`, constitution XXXI,
  `spec.md`, `plan.md`, and this tasks file; tick completed tasks accurately in
  `specs/vk/6df4-loading-chat-pin/tasks.md` (depends on T011).

## Phase 5: Review, knowledge, and delivery

- [x] T013 Run an independent Codex diff review, address every confirmed
  significant finding, rerun affected verification, and repeat until clean;
  record the result in `specs/vk/6df4-loading-chat-pin/review.md` (depends on
  T012).
- [x] T014 Update
  `docs/knowledge-base/lazy-loading-normalized-conversation-history.md` and
  `docs/knowledge-base/INDEX.md` with reusable single-flight materialization
  knowledge tagged `vk/6df4-loading-chat-pin`, then commit the knowledge-base
  update (depends on T013).
- [x] T015 Confirm the latest base tip and scoped diff, commit remaining changes,
  open a pull request, wait for required checks/review, merge it, and record the
  merged PR in `specs/vk/6df4-loading-chat-pin/verification.md` (depends on
  T014).
