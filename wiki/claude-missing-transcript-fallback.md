# Claude follow-ups when the transcript is gone

A Claude Code follow-up runs `claude --resume <agent_session_id>`. The
transcript lives only on the host that ran the session, at
`$CLAUDE_CONFIG_DIR|$HOME/.claude/projects/<slug>/<id>.jsonl`. When that
file is gone, Claude exits with `No conversation found with session ID:
<id>`. VK's session record still holds the old id, so before this fix
**every** later follow-up failed the same way and the workspace was stuck
(SWE-21, `vk/d97a-build-daily-repo`: transcript from 2026-07-07, deleted by
Claude's default 30-day cleanup before homelab raised retention on
2026-09-16).

## Behaviour (constitution XLVI)

`ClaudeCode::spawn_follow_up` probes first:

- **Present** (in any project folder) or **Unknown** → resume exactly as
  before, including `--resume-session-at`.
- **Missing** → `warn!`, then the *initial* command (no resume flags). The
  agent gets a notice ahead of the prompt to rebuild context from the
  files and git history. The user gets a `Vibe Kanban: …` line on stderr,
  which renders as an error entry in the turn. The new Claude session id
  replaces the old one through the normal `push_session_id` flow, so the
  next follow-up resumes the new conversation.

## Gotchas

- **Vendor lookup scope.** Verified against the pinned 2.1.281 binary:
  `--resume <id>` finds the transcript in *any* `projects/*` folder, not just
  the cwd's slug, and the lookup runs before the auth check. So the probe
  scans every folder and never recomputes the slug. Re-verify on a Claude
  bump (recipe: a throwaway `CLAUDE_CONFIG_DIR`, no credentials, and check
  whether the output is "No conversation found" or "Not logged in").
- **Resolve from the execution env.** Workers inject a scoped `HOME` via
  `ExecutionEnv.vars`, and profiles may set `CLAUDE_CONFIG_DIR`. Use
  `env.clone().with_profile(&self.cmd).vars` before the process env. A
  *relative* value resolves against the child's cwd, so treat it as Unknown.
- **Fail open, strictly.** Only a readable `projects/` with a clean
  NotFound/NotADirectory for every entry counts as Missing. An unreadable
  project folder or a failed `stat` gives Unknown (Codex review round 1
  caught `.flatten()` + `is_file()` quietly turning errors into "missing").
- **Notice channel.** A synthetic Claude `system/status` JSON line on
  stdout would render nicely but breaks constitution IX (VK metadata never
  goes in agent stdout). `stdout_dup::prepend_child_stderr` swaps in a
  stderr pipe, writes the notice, then copies the child's stderr through.
  EOF still follows the child.

## Rejected alternatives

- **Detect the error in the stream and respawn.** Claude has already exited
  inside a turn the container owns; a pre-spawn `stat` is simpler.
- **Seed the new conversation from VK's normalized logs.** Prior knowledge
  (`vk/6026-no-conversation`) forbids fabricating Claude-private history.
  A summary seed is a separate, deliberate recovery choice.

## Contributed by

- vk/9f5d-no-conversation
