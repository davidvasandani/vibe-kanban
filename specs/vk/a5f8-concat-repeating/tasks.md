# Tasks: Concatenate Repeating Lines

**Plan**: `./plan.md`

Tasks are dependency-ordered. Tasks marked **[P]** touch independent files and
may run together within their layer.

## Layer 1 — Repeat model and lifecycle

- [x] T001 Add bounded repeat-marker rendering, eligible-command recognition,
  `RepeatedCommand`, and per-command repeat count to
  `crates/executors/src/executors/codex/normalize_logs.rs`.
- [x] T002 Add shared command-start allocation and latest-owner completion
  helpers to `crates/executors/src/executors/codex/normalize_logs.rs`. Depends
  on T001.

## Layer 2 — Protocol integration

- [x] T003 Route direct app-server `CommandExecution` start/completion through
  the shared lifecycle helpers in
  `crates/executors/src/executors/codex/normalize_logs.rs`. Depends on T002.
- [x] T004 Route legacy `ExecCommandBegin`/`ExecCommandEnd` through the shared
  lifecycle helpers in
  `crates/executors/src/executors/codex/normalize_logs.rs`. Depends on T003.

## Layer 3 — Regression coverage

- [x] T005 Add focused direct-protocol tests for adjacent compaction, tick
  bounds, streaming updates, changed/intervening commands, non-review commands,
  and failed runs in
  `crates/executors/src/executors/codex/normalize_logs.rs`. Depends on T004.
- [x] T006 Add equivalent legacy-protocol coverage and patch-operation/index
  assertions in `crates/executors/src/executors/codex/normalize_logs.rs`.
  Depends on T005.

## Layer 4 — Verification

- [x] T007 Install locked dependencies with
  `pnpm install --frozen-lockfile`, run focused executor tests, and inspect
  failures. Depends on T006.
- [x] T008 Run `pnpm run format`, broader Rust checks/tests appropriate to the
  executor-only change, and inspect the diff for unrelated changes. Depends on
  T007.

## Layer 5 — Independent review and knowledge

- [x] T009 Run an independent Codex diff review, address confirmed significant
  findings, rerun validation, and repeat until clean. Depends on T008.
- [x] T010 [P] Update
  `docs/knowledge-base/collapsing-repeated-log-entries.md`,
  `docs/knowledge-base/claude-log-normalization.md` or a Codex-specific page as
  appropriate, and `docs/knowledge-base/INDEX.md`, tagging reusable knowledge
  with `a5f8-concat-repeating`; commit the knowledge-base update. Depends on
  T009.

## Parallel Execution Notes

The implementation and its tests share one Rust module and are intentionally
serial. The final knowledge update is file-independent from code but waits for
review so it records what actually shipped.
