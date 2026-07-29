# Tasks: Reliable Parallel Sub-Agent Pipeline

**Feature**: `specs/vk/a17f-fix-parallel-age/`
**Task**: `a17f-fix-parallel-age`

## Layer 1 — Executable prompt contract

- [x] T001 Rewrite `assets/pipelines/parallel-subagents.toml` to require
  concurrent named-provider launches, unchanged initial task delivery,
  workspace-reading tools under non-mutating policy, labeled complete outputs,
  and isolated failures.
- [x] T002 Refine analyze/iterate wording to prohibit fabricated substitutes and
  require fresh concurrent children with original-plus-synthesis context for at
  most `N` completed rounds. Depends on T001.

## Layer 2 — Safe bundle refresh

- [x] T003 Add the exact prior parallel-pipeline content as private historical
  migration data in `crates/services/src/services/pipelines/mod.rs`.
- [x] T004 Implement locked, atomic exact-content refresh during
  `ensure_seeded`, preserving absent and differing files. Depends on T003.

## Layer 3 — Regression coverage

- [x] T005 Strengthen bundled prompt assertions for initial prompt order, read
  capability, no all-tool disabling, concurrency, failure isolation, fresh
  children, non-substitution, and completed-round bounds. Depends on T001 and
  T002.
- [x] T006 Add exact-legacy upgrade, customized-file preservation, deleted-file
  preservation, and idempotence tests. Depends on T004.

T005 and T006 are `[P]` after their respective implementation dependencies,
though they touch the same Rust test module and should be edited together in
this worktree to avoid conflicts.

## Layer 4 — Verification

- [ ] T007 Run focused pipeline service tests. Depends on T005 and T006.
- [ ] T008 Run repository formatting and relevant crate checks, then inspect the
  diff for unrelated changes. Depends on T007.

## Layer 5 — Review and knowledge

- [ ] T009 Run independent Codex review, fix confirmed significant findings,
  and repeat focused verification/review until clean. Depends on T008.
- [ ] T010 Record reusable prompt-contract and exact-default-refresh knowledge,
  update the knowledge-base index, and commit the knowledge-base changes.
  Depends on T009.
