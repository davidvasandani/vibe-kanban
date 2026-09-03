# Tasks: Recover Missing Codex Conversations

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run together within their layer.

## Phase 1: Preserve and classify the protocol error

- [x] T001 Add a structured JSON-RPC response variant to `ExecutorError` while
  preserving existing diagnostic rendering in
  `crates/executors/src/executors/mod.rs`.
- [x] T002 Route pending JSON-RPC errors into the structured executor error and
  add focused response-conversion tests in
  `crates/executors/src/executors/codex/jsonrpc.rs` (depends on T001).
- [x] T003 Add the UUID-bound missing-conversation classifier and positive/
  negative unit cases in `crates/executors/src/executors/codex.rs` (depends on
  T001).

## Phase 2: Recover the normal chat follow-up

- [x] T004 Implement fork-or-start resolution for normal Codex chat follow-ups
  in `crates/executors/src/executors/codex.rs`, preserving the common
  registration and turn-start path (depends on T002, T003).
- [x] T005 Add focused behavior tests for classified fallback eligibility and
  fail-loud nonmatching errors in
  `crates/executors/src/executors/codex.rs`, while retaining the existing typed
  request-path coverage for successful start/fork responses (depends on T004).

## Phase 3: Verification

- [x] T006 Run focused `executors` tests for JSON-RPC conversion and Codex
  recovery behavior (depends on T005).
- [ ] T007 Run `pnpm install --frozen-lockfile`, `pnpm run format`, and the
  relevant executor/backend checks required by `AGENTS.md` (depends on T006).
- [ ] T008 Record verification evidence in
  `specs/vk/af0d-no-conversation/verification.md` (depends on T007).

## Phase 4: Review, knowledge, and merge

- [ ] T009 Run an independent Codex diff review, address confirmed findings,
  and repeat verification/review until no significant findings remain; record
  the result in `specs/vk/af0d-no-conversation/review.md` (depends on T008).
- [ ] T010 Update the reusable Vibe Kanban knowledge in
  `docs/knowledge-base/codex-rollout-transfer.md`, tag it
  `vk/af0d-no-conversation`, refresh `docs/knowledge-base/INDEX.md`, and commit
  the knowledge base (depends on T009).
- [ ] T011 Commit the remaining implementation and task artifacts
  intentionally, push `vk/af0d-no-conversation`, open a pull request against
  the latest base branch, pass required checks, and merge it (depends on T010).
