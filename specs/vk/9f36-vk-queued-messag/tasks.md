# Tasks: Queued Follow-up After No-change Run

**Plan**: `./plan.md`
**Task**: `vk/9f36-vk-queued-messag`

## Layer 0 - Baseline

- [x] T001 Trace the exit monitor and confirm the reported `0 files changed`
      branch manually finalizes under `already_finalized`, bypassing the normal
      `take_queued` block.

## Layer 1 - Implementation

- [x] T002 Extract shared queued-message scratch deletion and execution start
      behavior in `crates/local-deployment/src/container.rs`.
- [x] T003 Update the no-change cleanup-skip branch to dispatch an existing
      queued follow-up before manual finalization, falling back safely when the
      queue is empty/cancelled or start fails. Depends on T002.
- [x] T004 Reuse the helper from the normal queue consumer. Depends on T002.
- [x] T005 Add focused skipped-cleanup decision tests for queued and empty cases.
      Depends on T003.

## Layer 2 - Validation

- [x] T006 Run focused `local-deployment` queued-follow-up tests (2 passed).
- [x] T007 Run relevant backend check/test validation (`local-deployment` lib:
      28 passed).
- [x] T008 Run required `pnpm run format` and verify no unrelated churn.

## Layer 3 - Review and knowledge

- [x] T009 Run independent Codex CLI review; it reported no discrete correctness
      issues, and its independent `cargo check -p local-deployment` passed.
- [x] T010 Update the project knowledge base with the reusable early-finalization
      queue handoff rule, refresh the index, and commit the knowledge-base change
      (`18736e5e`).

## Parallelization Notes

The production changes share one exit-monitor block and are intentionally
sequential. Validation and review run against the complete formatted diff.
