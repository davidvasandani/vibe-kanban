# Tasks: Claude follow-ups survive a deleted transcript

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Building blocks
- [x] T001 [P] Add `prepend_child_stderr(child, notice)` (pipe swap, writes the notice, then copies the original stderr through) in `crates/executors/src/stdout_dup.rs`
- [x] T002 [P] Add `claude_config_dir`, `TranscriptStatus` and `claude_transcript_status`, plus the `MISSING_TRANSCRIPT_AGENT_NOTICE` and `missing_transcript_user_notice` text, in `crates/executors/src/executors/claude.rs`

## Phase 2: Wiring
- [x] T003 Add the `startup_notice: Option<String>` parameter to `spawn_internal` and call `prepend_child_stderr` after spawning; `spawn` passes `None`. In `crates/executors/src/executors/claude.rs` (depends on T001)
- [x] T004 Branch `spawn_follow_up` on the probe: `Missing` → `warn!`, `build_initial`, agent notice ahead of the prompt, user notice to `spawn_internal`; otherwise unchanged `--resume`. In `crates/executors/src/executors/claude.rs` (depends on T002, T003)

## Phase 3: Validation
- [x] T005 [P] Unit test the stderr ordering and EOF in `crates/executors/src/stdout_dup.rs` (depends on T001)
- [x] T006 [P] Unit test the probe outcomes and config-dir precedence in `crates/executors/src/executors/claude.rs` (depends on T002)
- [x] T007 Run `cargo test -p executors`, `cargo clippy -p executors --all-targets` and `pnpm run format`; fix any findings (depends on T004–T006)
