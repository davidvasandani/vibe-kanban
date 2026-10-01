# Implementation plan: Claude follow-ups survive a deleted transcript

Task: `vk/9f5d-no-conversation`. See `SPEC.md` and `PRIOR_KNOWLEDGE.md`.

Changes are in `crates/executors/src/executors/claude.rs` and
`crates/executors/src/stdout_dup.rs`.

## Steps

1. **Config-dir resolution helper.**
   `fn claude_config_dir(vars: &HashMap<String, String>) -> Option<PathBuf>`:
   look up `CLAUDE_CONFIG_DIR` (non-empty) and then `HOME` → `HOME/.claude`.
   For each variable, check `vars` (execution env + profile env) before the
   process env.

2. **Transcript probe.**
   `fn transcript_status(config_dir: &Path, session_id: &str) -> TranscriptStatus`
   returns `Present`, `Missing` or `Unknown`.
   - `Unknown` when the session id isn't one safe file-name segment (empty,
     `/`, `\`, `..`, NUL) or when `config_dir/projects` can't be read.
   - `Present` when any `projects/<dir>/<id>.jsonl` exists. Errors reading
     an individual entry are skipped.
   - `Missing` otherwise.

3. **Follow-up wiring** in `ClaudeCode::spawn_follow_up`:
   - Compute `env.clone().with_profile(&self.cmd)` vars, resolve the config
     dir and probe.
   - On `Missing`: `tracing::warn!`, then build the *initial* command, put
     `MISSING_TRANSCRIPT_AGENT_NOTICE` in front of the prompt, and pass a
     user-facing notice to `spawn_internal`.
   - Otherwise keep today's behaviour (`--resume`, optional
     `--resume-session-at`).

4. **Visible notice on stderr (constitution IX).** VK must not inject its
   own metadata into the agent's stdout, so the notice can't be a fake
   Claude JSON line. Add `prepend_child_stderr(child, notice)` to
   `stdout_dup.rs`. It takes the child's stderr, puts in a fresh pipe, and
   runs a task that writes the notice first and then copies the original
   stderr through. `normalize_claude_stderr_logs` already renders stderr
   as a visible error entry, which is the same channel worker diagnostics
   use. `spawn_internal` gains an `Option<&str>` startup notice and calls
   the helper right after spawning.

5. **Tests** (`#[cfg(test)]` in `claude.rs`, using `tempfile`):
   - present in some project dir → `Present`
   - projects dir exists, file absent → `Missing`
   - no projects dir → `Unknown`
   - unsafe ids (`""`, `"../x"`, `"a/b"`) → `Unknown`
   - `CLAUDE_CONFIG_DIR` in vars wins over `HOME`; `HOME` maps to `.claude`
   - `prepend_child_stderr` on a real child (`sh -c 'echo inner >&2'`)
     yields the notice line before `inner`, and EOF once the child exits.

6. **Verify:** `cargo test -p executors claude`,
   `cargo clippy -p executors --all-targets`, `pnpm run format`.

7. **Wiki** (stage 12): new page `wiki/claude-missing-transcript-fallback.md`
   plus an index line.
