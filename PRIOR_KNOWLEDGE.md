# Prior knowledge: `No conversation found with session ID`

Task: `vk/9f5d-no-conversation`. Read-only recall from the project knowledge
bases (`vibe-kanban/wiki/`, `homelab/docs/knowledge-base/`).

## Directly relevant

### homelab `vibe-kanban-claude-transcript-retention.md` (`vk/6026-no-conversation`)

- An earlier incident hit the same error (a workspace idle from Jul 27 to
  Sep 2). The Vibe session and its normalized UI logs live in VK's DB.
  Claude's private transcript is separate, at
  `~/.claude/projects/<slug>/<id>.jsonl`.
- Cause: Claude Code's default `cleanupPeriodDays` is 30 days, so idle
  transcripts get deleted. Homelab now sets `claudeTranscriptRetentionDays`
  (default 3650) via the `vibe-kanban-claude-retention` oneshot, for both
  the coordinator and the workers.
- **Recovery boundary:** retention doesn't bring back deleted JSONL files.
  "Do not fabricate Claude-private history from normalized UI logs or
  silently substitute an empty session. A deliberately new conversation …
  is a separate recovery choice."
  → **Constraint for this task:** a fresh-session fallback must be visible
  to the user (a system message in the chat) and to the agent (a notice in
  its prompt). It must never pretend to be the original conversation.

### wiki `agent-process-lifecycle.md`

- Follow-ups re-invoke Claude with `--resume <agent_session_id>`. ACP
  resume replays a `.jsonl` transcript into a fresh process. Nothing there
  covers a transcript going missing.

### Codex precedent (code, not wiki)

- `crates/executors/src/executors/codex.rs` `classify_fork_rejection`:
  `ConversationMissing` / `LineageUnusable` → start a replacement thread in
  the same workspace and log `warn`. This is the precedent for falling
  back automatically.

## Worker / scoped-home facts (code)

- Workers give each execution a scoped `HOME`
  (`crates/worker/src/execution.rs` `prepare_scoped_home`). It symlinks
  every entry of the real home, including `.claude/projects`, so
  transcripts are shared across executions on the same host. The scoped
  `HOME` reaches the executor through `ExecutionEnv.vars`
  (`env.vars.extend(environment)`).
- So the executor must resolve the config dir from the execution env
  merged with the profile env, not from its own process `HOME`.

## Gaps

- No wiki page covers executor-side handling of a missing transcript. That
  page gets written in stage 12.
