# Tasks: reliable Claude background-Bash denial

**Plan**: `./plan.md`

## Phase 1: Base and reproduction

- [x] T001 Merge current `origin/main` into the task branch and reconcile only
      this task's root/SpecKit artifacts.
- [x] T002 Add an internal testable async-I/O seam in
      `crates/executors/src/executors/claude/protocol.rs` without changing the
      production Claude child-pipe interface (depends on T001).
- [x] T003 Add a deterministic failing regression in
      `crates/executors/src/executors/claude/protocol.rs`: terminal result,
      continuing post-result activity near the original deadline, then
      `DENY_BACKGROUND_BASH_CALLBACK_ID`; assert the matching response is
      flushed (depends on T002).

## Phase 2: Lifecycle correction

- [x] T004 Change terminal grace in
      `crates/executors/src/executors/claude/protocol.rs` from an absolute
      post-result deadline to an idle deadline refreshed by every subsequent
      non-empty output line (depends on T003).
- [x] T005 Ensure mandatory hook-response write failures return/log an explicit
      protocol failure without altering approval-cancellation behavior in
      `crates/executors/src/executors/claude/protocol.rs` (depends on T004).

## Phase 3: Coverage and artifacts

- [x] T006 [P] Add/adjust focused tests for true-quiescence shutdown,
      cancellation, and callback response semantics in
      `crates/executors/src/executors/claude/protocol.rs` and
      `crates/executors/src/executors/claude/client.rs` (depends on T005).
- [x] T007 [P] Update task verification notes under
      `specs/vk/5cd1-debug-this-vk-ba/` with the reproduced ordering and exact
      commands (depends on T005).

## Phase 4: Verification and review

- [x] T008 Run focused executor tests and Rust formatting; run broader checks
      proportionate to the final diff (depends on T006).
- [x] T009 Run independent Codex diff review, address every confirmed
      significant finding, and rerun affected checks until clean (depends on
      T008).
- [x] T010 Update the existing lifecycle/poller knowledge page and index with
      reusable VAS-540 findings, tagged `vk/5cd1-debug-this-vk-ba` (depends on
      T009).

## Phase 5: Delivery

- [ ] T011 Commit and push the Vibe Kanban changes, open a pull request against
      the base branch, and verify required CI (depends on T010).
- [ ] T012 Merge the pull request after CI/review is clear and record the merged
      PR and commit in verification notes (depends on T011).

## Dependency graph

```text
T001 → T002 → T003 → T004 → T005 → {T006, T007} → T008 → T009 → T010 → T011 → T012
```

Only T006 and T007 are parallel-safe; all source lifecycle changes share one
file and must remain serial.
