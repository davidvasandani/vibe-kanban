# Tasks: Add Claude Opus 5 to Executor Model Selectors

**Feature**: `specs/vk/3137-update-vk-for-op/`
**Plan**: `specs/vk/3137-update-vk-for-op/plan.md`

## Layer 1 — Source edits (parallel)

All four executor edits touch independent files with no shared state.

- [x] T001 [P] In `crates/executors/src/executors/claude.rs`, insert
  `("claude-opus-5", "Opus 5")` into the model catalog array in
  `default_discovered_options()` (line ~283) after `("opus[1m]", "Opus (1M context)")`
  and before `("claude-sonnet-5", "Sonnet 5")`.

- [x] T002 [P] In `crates/executors/src/executors/cursor.rs`, make four
  coordinated edits:
  (a) Insert `opus-5` into the `#[schemars(description)]` string on
  `CursorAgent.model` (line ~50) after `auto` and before `opus-4.8`.
  (b) Add match arms `("opus-5", Some("standard")) => "opus-5"` and
  `("opus-5", Some("thinking") | None) => "opus-5-thinking"` in
  `resolve_cursor_model_name()` before the `opus-4.8` arms (line ~99).
  (c) Prepend `"opus-5"` to the reasoning-options match arm in
  `cursor_reasoning_options()` (line ~125).
  (d) Insert `("opus-5", "Claude 5 Opus")` before `("opus-4.8", ...)`
  in `discover_options()` (line ~656).

- [x] T003 [P] In `crates/executors/src/executors/copilot.rs`, insert
  `("claude-opus-5", "Claude Opus 5")` before `("claude-opus-4.8", ...)`
  in `discover_options()` (line ~201).

- [x] T004 [P] In `crates/executors/src/executors/droid.rs`, make two edits:
  (a) Add `claude-opus-5` to the examples in the `#[schemars(description)]`
  string on `Droid.model` (line ~72).
  (b) Insert `("claude-opus-5", "Claude Opus 5")` before
  `("claude-opus-4-8", ...)` in `discover_options()` (line ~241).

## Layer 2 — Focused tests (parallel, depends on respective Layer 1 task)

Each test file is the same file as its corresponding source edit, so each
depends on exactly one Layer 1 task. The four test tasks are independent of
each other.

- [x] T005 [P] Add a unit test in `claude.rs` `mod tests` (after line ~2861)
  verifying `default_discovered_options()` contains `"claude-opus-5"` with
  display name `"Opus 5"` and non-empty reasoning options (confirming
  `supports_effort` coverage). Depends on T001.

- [x] T006 [P] Add tests in `cursor.rs` `mod tests` (after line ~1404):
  (a) `resolve_cursor_model_name` returns `"opus-5"` for standard and
  `"opus-5-thinking"` for thinking/None.
  (b) `cursor_reasoning_options("opus-5")` returns two options (standard,
  thinking).
  (c) `discover_options()` output contains `"opus-5"`.
  Depends on T002.

- [x] T007 [P] Add a `#[cfg(test)] mod tests` block in `copilot.rs` with a
  test verifying `discover_options()` output contains `"claude-opus-5"`.
  Depends on T003.

- [x] T008 [P] Add a `#[cfg(test)] mod tests` block in `droid.rs` with a
  test verifying `discover_options()` output contains `"claude-opus-5"`.
  Depends on T004.

## Layer 3 — Generated artifact regeneration

Must follow all source edits (Layer 1) since schema metadata changed.

- [x] T009 Run `pnpm run generate-types` to regenerate `shared/schemas/*.json`
  and `shared/types.ts`. Verify that `shared/schemas/cursor_agent.json` and
  `shared/schemas/droid.json` gained the new model references, and that
  `shared/schemas/claude_code.json`, `shared/schemas/copilot.json`, and
  `shared/types.ts` are unchanged. Depends on T001, T002, T003, T004.

## Layer 4 — Verification (parallel where noted)

- [x] T010 [P] Run `cargo test --workspace` to execute all focused tests and
  confirm no regressions. Depends on T005, T006, T007, T008, T009.

- [x] T011 [P] Run `pnpm run generate-types:check` to confirm generated
  artifacts are in sync with source. Depends on T009.

- [x] T012 Run `pnpm run format` to format all Rust and web code. Depends on
  T010, T011.

- [x] T013 Run `pnpm run lint` (ESLint + cargo clippy) and `pnpm run check`
  (frontend type checks + backend Rust checks). Depends on T012.

## Layer 5 — Independent review

- [x] T014 Run an independent Codex review of the complete diff
  (`git diff main...HEAD`). Address all confirmed significant findings, rerun
  relevant Layer 4 checks, and repeat review until clean. Depends on T013.

## Layer 6 — Knowledge-base enrichment

- [x] T015 Create or update `docs/knowledge-base/executor-model-catalog-maintenance.md`
  documenting the reusable procedure for adding a new model to executor catalogs
  (which files, which sections, schema regeneration, test patterns). Tag with
  `3137-update-vk-for-op`. Update `docs/knowledge-base/INDEX.md` with a pointer
  to the new entry. Depends on T014.

## Parallel execution notes

- T001, T002, T003, T004 are fully independent (separate files, no shared
  state) and may all run in parallel.
- T005, T006, T007, T008 each depend on exactly one Layer 1 task and edit the
  same file as that task. They are independent of each other and may run in
  parallel once their respective Layer 1 dependency is complete.
- T010 and T011 are read-only checks on different toolchains and may run in
  parallel.
- T012 must follow T010/T011 to avoid formatting unstaged test failures.
- T013 must follow T012 to lint already-formatted code.
- T014 and T015 are strictly sequential — review before knowledge capture.
