# Tasks: Restore Opus 4.8 in the Claude Code model picker

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes. T001–T004 all edit `crates/executors/src/executors/claude.rs`, so they
are serial.

## Phase 1: Tests first (contract)
- [x] T001 Update `test_claude_discovered_catalog_is_versioned_and_defaults_to_opus_5_5`
  to expect 10 entries with `("claude-opus-4-8", "Opus 4.8", &efforts)` after
  Opus 5, in `crates/executors/src/executors/claude.rs` (FR-1, FR-2, FR-3)
- [x] T002 Update `test_context_window_for_model` so `claude-opus-4-8` asserts
  `CLAUDE_1M_CONTEXT_WINDOW`, in `crates/executors/src/executors/claude.rs`
  (FR-4; depends on T001, same file)

## Phase 2: Core
- [x] T003 Insert `("claude-opus-4-8", "Opus 4.8")` after `("claude-opus-5", "Opus 5")`
  in `default_discovered_options()`, in
  `crates/executors/src/executors/claude.rs` (FR-1–FR-3; depends on T001)
- [x] T004 Add `"claude-opus-4-8"` to the 1M arm of `context_window_for_model()`
  in `crates/executors/src/executors/claude.rs` (FR-4; depends on T002)

## Phase 3: Validation
- [x] T005 Run `cargo test -p executors` and confirm T001/T002 and the alias test
  pass; no file changes (depends on T003, T004)
- [x] T006 [P] Run `pnpm run generate-types:check` and confirm `shared/types.ts`
  is unchanged (FR-5 shape check; depends on T003)
- [x] T007 [P] Run `pnpm run format` and `cargo clippy -p executors`; commit any
  formatting changes in `crates/executors/src/executors/claude.rs` (depends on T004)
