# Implementation plan: Claude follow-ups survive a deleted transcript

Task: `vk/9f5d-no-conversation`. See `SPEC.md` and `PRIOR_KNOWLEDGE.md`.

All changes are in `crates/executors/src/executors/claude.rs`.

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

4. **Visible notice.** Add an `Option<String>` startup-notice parameter to
   `spawn_internal`. Inside the spawned task, before control-protocol
   initialization, write a serialized
   `{"type":"system","subtype":"status","status":<notice>}` line through
   `log_writer.log_raw`. The existing normalizer turns `status` system
   messages into a `SystemMessage` entry. Build it with `serde_json` so the
   text is escaped.

5. **Tests** (`#[cfg(test)]` in `claude.rs`, using `tempfile`):
   - present in some project dir → `Present`
   - projects dir exists, file absent → `Missing`
   - no projects dir → `Unknown`
   - unsafe ids (`""`, `"../x"`, `"a/b"`) → `Unknown`
   - `CLAUDE_CONFIG_DIR` in vars wins over `HOME`; `HOME` maps to `.claude`
   - the notice line parses as `ClaudeJson::System` with subtype `status`
     and normalizes to a `SystemMessage` with the notice text.

6. **Verify:** `cargo test -p executors claude`,
   `cargo clippy -p executors --all-targets`, `pnpm run format`.

7. **Wiki** (stage 12): new page `wiki/claude-missing-transcript-fallback.md`
   plus an index line.
