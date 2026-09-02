# Tasks: Turn completion clears the running composer

**Plan**: `./plan.md`

Tasks are dependency ordered. The core regression and implementation share one
Rust module, so they are intentionally serial rather than marked `[P]`.

## Phase 1: Regression contract

- [x] T001 Update the paused-time final-output reconciliation tests to cover a
  live natural-exit child and a live signal-driven child in
  `crates/local-deployment/src/container.rs`.

## Phase 2: Core lifecycle fix

- [x] T002 Qualify live-child turn evidence by executor terminal mechanism in
  `wait_for_unfinalized_output` and `spawn_exit_monitor` in
  `crates/local-deployment/src/container.rs` (depends on T001).

## Phase 3: Verification

- [x] T003 Run focused and crate-level Rust tests for
  `crates/local-deployment/src/container.rs` and fix any regressions (depends on
  T002).
- [x] T004 Run repository formatting and required checks for the files governed
  by `AGENTS.md` (depends on T003).

## Phase 4: Delivery

- [x] T005 [P] Record implementation/verification evidence in
  `specs/vk/7655-turn-ends-aren-t/verification.md` (depends on T004).
- [ ] T006 Run independent Codex review and record the clean result in
  `specs/vk/7655-turn-ends-aren-t/review.md` (depends on T004).
- [ ] T007 Update the relevant reusable lifecycle topic and index in
  `wiki/agent-process-lifecycle.md` and `wiki/INDEX.md`, tagged with
  `vk/7655-turn-ends-aren-t` (depends on T006).
- [ ] T008 Open and merge the pull request after the implementation, review,
  verification, and knowledge commits are ready (depends on T005, T007).
