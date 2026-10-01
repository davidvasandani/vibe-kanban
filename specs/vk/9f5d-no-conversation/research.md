# Research: Claude follow-ups survive a deleted transcript

## Incident evidence (2026-10-01)

- Workspace `d97a303b-cf31-447a-b848-e7604c0a67ef`
  (`vk/d97a-build-daily-repo`), VK session `4c9c88ce-…`, executor
  `CLAUDE_CODE`.
- Failed execution `8dfca28c-3018-4f97-9b9d-503bdca48165`:
  `CodingAgentFollowUpRequest { session_id: "f54d7e57-…", reset_to_message_id: null }`,
  exit 1 in about 6 s, only output `No conversation found with session ID: f54d7e57-…`.
- Last successful agent turn: 2026-07-07. The homelab retention fix
  (#1228, `cleanupPeriodDays` → 3650) landed 2026-09-16, after the
  transcript was already gone. `f54d7e57*.jsonl` isn't on think1 under
  `/var/lib/vibe-kanban/.claude/projects` or any scoped worker home.

## Vendor behaviour, verified against the pinned binary

Verified against `@anthropic-ai/claude-code@2.1.281` (the pin in
`claude.rs::base_command`), native `claude-code-linux-x64/claude`, with
throwaway config dirs and no credentials:

| Setup | Output of `claude --resume <id> -p hi` |
| --- | --- |
| `CLAUDE_CONFIG_DIR=$T/cfg`, empty `projects/` | `No conversation found with session ID: <id>` |
| transcript at `$T/cfg/projects/-elsewhere/<id>.jsonl` (cwd is a different project) | `Not logged in · Please run /login` (lookup passed) |
| transcript at `$T/cfg/projects/<cwd-slug>/<id>.jsonl` | `Not logged in …` (lookup passed) |
| `HOME=$T/home`, no `CLAUDE_CONFIG_DIR`, transcript at `$T/home/.claude/projects/-elsewhere/<id>.jsonl` | `Not logged in …` (lookup passed) |

Conclusions:

1. The session lookup runs before authentication, and its message matches
   the incident exactly.
2. `--resume <id>` finds a transcript in **any** project folder, not just
   the cwd's. The probe therefore scans every `projects/*/` folder for
   `<id>.jsonl` and doesn't recompute Claude's slug algorithm. That keeps
   the probe in step with the vendor and avoids copying an undocumented
   slug rule.
3. Root resolution: `CLAUDE_CONFIG_DIR`, else `$HOME/.claude`.

## Decisions

- **Probe before spawning, not retry after failure.** Claude exits after
  printing the error, so retrying would mean detecting the failure from
  the stream, killing the child and spawning again inside a turn the
  container already owns. A filesystem existence check is cheap
  (dozens of directory entries on local disk) and leaves the spawn path
  linear. Rejected: parse-and-retry.
- **Fail open.** Only a readable `projects/` folder with no match counts as
  missing (constitution XLVI, XV's "an error is never evidence").
- **Resolve from the execution env.** Workers inject a scoped `HOME`
  through `ExecutionEnv.vars` (`crates/worker/src/execution.rs`,
  `env.vars.extend(environment)`), and profiles can set
  `CLAUDE_CONFIG_DIR`. The probe reads
  `env.clone().with_profile(&self.cmd).vars` first, then the process env,
  the same precedence the child sees through `apply_to_command`.
- **User notice on stderr.** Constitution IX forbids writing VK metadata to
  agent stdout (a synthetic `system/status` Claude line was considered and
  rejected for that reason). `normalize_claude_stderr_logs` renders stderr
  as a visible error entry, which is the channel worker diagnostics already
  use (`wiki/worker-journal-agent-stream-boundary.md`). A small
  `prepend_child_stderr` helper next to `create_stdout_pipe_writer` swaps
  in a pipe, writes the notice, then copies the real stderr through.
- **Reset point with a missing transcript → fresh** (spec clarification).
- **No new dependencies.** `tempfile` is already a dev-dependency, and
  `os_pipe` / `tokio` are already used in `stdout_dup.rs`.
