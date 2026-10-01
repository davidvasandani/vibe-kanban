# Implementation Plan: Claude follow-ups survive a deleted transcript

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

Rust (workspace toolchain), crate `executors`. The Claude Code executor is
`crates/executors/src/executors/claude.rs` (`ClaudeCode::spawn_follow_up`,
`ClaudeCode::spawn_internal`). Child pipe helpers are in
`crates/executors/src/stdout_dup.rs`. The pinned CLI is
`@anthropic-ai/claude-code@2.1.281`. No DB, API-type or frontend changes.

## Architecture & Approach

1. **Resolve the config root** (FR-1). New private
   `fn claude_config_dir(vars: &HashMap<String, String>) -> Option<PathBuf>`
   in `claude.rs`: non-empty `CLAUDE_CONFIG_DIR`, else non-empty `HOME` +
   `.claude`. Each variable is read from `vars` first, then `std::env`.
   The process-env lookup is split into a `lookup` closure, so tests
   don't depend on the test runner's env.
2. **Probe** (FR-1, FR-3). New
   `enum TranscriptStatus { Present, Missing, Unknown }` and
   `fn claude_transcript_status(config_dir: &Path, session_id: &str) -> TranscriptStatus`:
   - `Unknown` if the id is empty, `.`/`..`, or contains `/`, `\` or NUL;
   - `Unknown` if `read_dir(config_dir/projects)` fails;
   - `Present` if any entry's `path/<id>.jsonl` is a file (via
     `Path::is_file`, which treats errors as false);
   - `Missing` otherwise.
3. **Follow-up wiring** (FR-2, FR-4, FR-6, FR-7). In `spawn_follow_up`:
   `let vars = env.clone().with_profile(&self.cmd).vars;` →
   `claude_config_dir(&vars)` → probe. On `Missing`:
   - `tracing::warn!(missing_session_id = %session_id, config_dir = ?dir, "Claude transcript missing; starting a new conversation in the same workspace")`;
   - `build_initial()` (no `--resume`/`--resume-session-at`);
   - prompt = `MISSING_TRANSCRIPT_AGENT_NOTICE` + `"\n\n"` + prompt;
   - pass `Some(missing_transcript_user_notice(session_id))` to
     `spawn_internal`.
   Otherwise today's path, unchanged. The new Claude session id is picked
   up by the existing `extract_session_id` → `push_session_id` flow (FR-7).
4. **Visible notice** (FR-5). `spawn_internal` gains
   `startup_notice: Option<String>`. Right after `group_spawn_no_window`,
   when it's `Some`, call
   `crate::stdout_dup::prepend_child_stderr(&mut child, notice)`. The
   helper takes `child.inner().stderr`, creates an `os_pipe` pair, installs
   the reader as the new `ChildStderr`, and spawns a task that writes
   `notice + "\n"` and then runs `tokio::io::copy(original, writer)`.
   The writer drops at EOF, so the stream still ends when the child exits.
   The other callers (`spawn`) pass `None`.
5. **Tests** (`#[cfg(test)]`):
   - `claude.rs`: present in some project dir / absent / no projects dir /
     unsafe ids; `CLAUDE_CONFIG_DIR` precedence; `HOME` mapping; empty
     values ignored.
   - `stdout_dup.rs` (unix): spawn `sh -c 'echo inner >&2'` and apply the
     helper. Reading stderr yields `notice\ninner\n`, then EOF.

## Data Model

No persisted data changes. `data-model.md` and `contracts/` aren't needed:
no API, schema or generated-type surface changes.

## Contracts

None.

## Research Notes

See `./research.md` (vendor behaviour verified against the pinned 2.1.281
binary, rejected alternatives).

## Constitution Check

- **I / III (clarity, small steps):** two small pure helpers and a single
  branch in `spawn_follow_up`. This generalises the Codex precedent
  (`ForkRejection::ConversationMissing`) to Claude.
- **II (test the contract):** unit tests for every probe outcome and for
  the stderr ordering.
- **IX (agent protocols):** the vendor behaviour (lookup location, message,
  ordering before auth) was verified against the pinned executing artifact
  and the version is recorded. The VK notice goes to stderr, never agent
  stdout.
- **XV (fail safe, loud):** an unreadable location resumes as before.
  `warn!` comes before acting.
- **XXV (continuation artifacts):** read-only existence check. Nothing is
  copied, moved or deleted, and no symlinks are followed beyond what
  `is_file` resolves inside the projects folder.
- **XLVI (lost continuation):** implemented as stated.
- **Constraints:** no new dependencies, no generated files, and
  `pnpm run format` runs before completion.

No deviations.

## Risks & Dependencies

- **A future Claude version changes the transcript location.** Then the
  probe sees an empty or missing folder. A missing folder → `Unknown` →
  resume (today's behaviour). Present but stale → `Missing` → a fresh
  session that could have resumed. That is mitigated by the version pin:
  bumps are human-reviewed per AGENTS.md, and research.md records the
  verified version.
- **Transcript on a different worker than the one running the turn.** The
  probe reports `Missing` and starts fresh. The workspace would have failed
  anyway; the notice makes the outcome visible.
- **Large projects folder.** It is one `read_dir` plus one `stat` per
  entry: tens of entries in production, on local disk.
