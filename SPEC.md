# SPEC: Claude follow-ups survive a deleted session transcript

Task: `vk/9f5d-no-conversation`.

## Problem

Workspace `vk/d97a-build-daily-repo` (SWE-21, Claude Code session
`4c9c88ce`) can no longer take a follow-up. Every message fails within a
few seconds with:

```
No conversation found with session ID: f54d7e57-b912-487e-9d3b-50f3f6d7dd94
```

The follow-up execution (`8dfca28c`, 2026-10-01 10:53Z) ran
`claude --resume f54d7e57-…` and exited 1. The same thing happened on
2026-09-30.

## Root cause

- Claude Code stores each conversation as
  `$CLAUDE_CONFIG_DIR|$HOME/.claude/projects/<cwd-slug>/<session-id>.jsonl`.
  `--resume <id>` fails with the message above when that file is missing.
- The last successful turn was on 2026-07-07. Claude Code's default
  `cleanupPeriodDays` is 30, so it deleted the transcript around
  2026-08-06.
- The homelab retention fix (`claudeTranscriptRetentionDays`, default 3650,
  homelab #1228) only landed on 2026-09-16. It stops future deletions but
  can't restore files already deleted. Transcripts can also be lost when a
  workspace moves to a worker that never received them.
- Vibe Kanban still holds `f54d7e57…` as the session's agent session id.
  Each follow-up passes it to `--resume`, so the workspace stays stuck no
  matter what the user sends.
- Codex already handles this case: when its rollout is missing it starts a
  replacement thread in the same workspace (`ForkRejection::ConversationMissing`
  in `codex.rs`). The Claude executor has no equivalent.

## Goal

When a Claude Code follow-up names a session whose transcript is
provably missing, start a fresh Claude session in the same workspace
instead of failing. The next follow-up then resumes the new session, since
VK records the new session id from the stream as it already does today.

## Requirements

1. **FR-1 Detect a missing transcript before spawning.** Resolve Claude's
   config dir the same way the child process will see it:
   `CLAUDE_CONFIG_DIR`, else `$HOME/.claude`. Each variable is read from the
   execution env merged with the profile env first, then from the process
   env. The transcript counts as missing only when `<config>/projects` can
   be listed and none of its project subdirectories contains
   `<session-id>.jsonl`.
2. **FR-2 Fail open.** If the projects dir can't be read (missing dir,
   permission error, no HOME), resume as today, so a broken probe never
   discards a valid session. Session ids that aren't one safe path segment
   (empty, containing `/` or `..`) also resume as today.
3. **FR-3 Fresh session fallback.** When the transcript is missing, spawn
   the initial command (no `--resume`, no `--resume-session-at`). Put a
   short notice in front of the user's prompt saying the earlier
   conversation could not be restored and the agent should check the
   workspace's files and git history for prior work.
4. **FR-4 Visible, never silent.** Log a `warn` with the missing session id
   when falling back. Also write a Vibe Kanban diagnostic line to the
   execution's stderr ahead of Claude's own stderr. Per constitution IX it
   must never be injected into agent stdout. The chat then shows the user that the earlier Claude
   transcript was missing and a new conversation was started. Prior
   knowledge (`vk/6026-no-conversation`) forbids silently substituting an
   empty session.
5. **FR-5 Scope.** This change touches only the Claude Code executor (plus a
   generic stderr helper in `stdout_dup.rs`).
   Other executors, the DB schema and the frontend stay unchanged.

## Non-goals

- Recovering the deleted transcript, or rebuilding it from VK's stored
  logs.
- Changing transcript retention (homelab already owns that).
- Cross-worker transcript transfer (`session-transfers` already owns that).

## Acceptance

- Unit tests: transcript present (in any project dir) → resume; present
  under `CLAUDE_CONFIG_DIR` → resume; projects dir present but file absent
  → fresh; projects dir unreadable or missing → resume; unsafe session id →
  resume. The stderr notice comes before the child's own stderr output.
- `cargo test -p executors` passes; `pnpm run format` leaves the tree clean;
  clippy is clean for the crate.
