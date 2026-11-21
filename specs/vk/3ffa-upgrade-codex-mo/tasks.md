# Tasks: Upgrade Codex Models

**Plan**: `./plan.md`

Tasks are ordered by dependency. No product-code tasks are parallel-safe because
the implementation and regression assertion share one Rust file.

## Phase 1: Catalog Contract

- [x] T001 Replace the Codex discovery catalog and reasoning-option assignments
      in `crates/executors/src/executors/codex.rs`.
- [x] T002 Update the exact ordered catalog regression assertion in
      `crates/executors/src/executors/codex.rs` (depends on T001).

## Phase 2: Validation

- [x] T003 Run repository formatting, the focused Codex test, relevant executor
      checks, generated-type verification, and diff checks; record results in
      `specs/vk/3ffa-upgrade-codex-mo/verification.md` (depends on T002).
- [x] T004 Run independent Codex diff review, resolve confirmed findings, rerun
      affected verification, and record the clean review in
      `specs/vk/3ffa-upgrade-codex-mo/review.md` (depends on T003).

## Phase 3: Knowledge and Delivery

- [x] T005 Distill the reusable pinned-catalog compatibility lesson and add this
      task ID in `wiki/executor-model-catalogs.md`; update `wiki/INDEX.md` only
      if its existing summary needs a material change (depends on T004).
- [x] T006 Commit task changes in the `vibe-kanban` repository, push the task
      branch, open a pull request against the base branch, wait for required
      checks, resolve failures, and merge it (depends on T005).
