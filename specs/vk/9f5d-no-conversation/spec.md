# Feature Specification: Claude follow-ups survive a deleted transcript

**Feature dir**: `specs/vk/9f5d-no-conversation/`
**Status**: Clarified

## Summary

Claude Code keeps each conversation in a private transcript file on the
host that ran it. When that file is gone (Claude's own 30-day cleanup, a
host change, or manual deletion), every follow-up in the workspace fails
within seconds with `No conversation found with session ID: <id>`. Nothing
the user sends can recover it. Workspace `vk/d97a-build-daily-repo` (SWE-21)
is in that state: its transcript from 2026-07-07 was cleaned up around
2026-08-06, and follow-ups failed on 2026-09-30 and 2026-10-01. When the
transcript is provably missing, the follow-up should start a new Claude
conversation in the same workspace and say plainly that it did. That is
better than leaving the workspace stuck, and it matches what Codex already
does.

## User Stories

- As a user returning to an old Claude workspace, I want my follow-up to
  run, not fail with an error I can't act on, so the work can continue.
- As a user, I want to be told when the agent has lost its earlier
  conversation, so I don't assume it remembers context it no longer has.
- As the agent in the new conversation, I want to be told the earlier
  conversation is gone, so I rebuild context from the workspace's files and
  git history and don't guess.
- As an operator, I want a log record naming the missing session, so I can
  tell these fallbacks from other failures.

## Functional Requirements

- FR-1: Before a Claude follow-up resumes a session, the system checks
  whether that session's transcript exists, in the location the launched
  agent will actually read for this execution.
- FR-2: If the transcript is provably missing, the follow-up starts a new
  Claude conversation in the same workspace with the user's message,
  instead of attempting the resume.
- FR-3: If the check is inconclusive (location unreadable or unresolvable,
  or a malformed session id), the follow-up resumes exactly as today.
- FR-4: When FR-2 applies, the agent receives a short notice ahead of the
  user's message. It says the earlier conversation could not be restored
  and tells the agent to check the workspace's files and git history.
- FR-5: When FR-2 applies, the user sees a Vibe Kanban notice in that
  turn's conversation. It names the missing session id and says a new
  conversation was started. The notice is a Vibe Kanban diagnostic, not
  something presented as agent output.
- FR-6: When FR-2 applies, a warning is logged with the missing session id.
- FR-7: After the fallback, the next follow-up continues the new
  conversation (the new session id replaces the old one through the
  existing session tracking).
- FR-8: Other executors' behaviour is unchanged.

## Out of Scope

- Recovering the deleted transcript, or rebuilding Claude-private history
  from Vibe Kanban's stored logs.
- Transcript retention settings (homelab's `claudeTranscriptRetentionDays`).
- Cross-worker transcript transfer (existing `session-transfers`).
- Handling a resume that fails for reasons other than a missing transcript.
- Seeding the new conversation with a summary of the old one.

## Acceptance Criteria

- [ ] With the transcript present in any Claude project folder, the
      follow-up resumes (and keeps a requested reset point).
- [ ] With the project folder readable but the transcript absent, the
      follow-up starts a new conversation and gets the agent notice.
- [ ] With the project folder missing or unreadable, the follow-up resumes.
- [ ] Session ids that aren't one safe path segment resume (no probing
      outside the projects folder).
- [ ] The location is resolved from the execution's own environment
      (`CLAUDE_CONFIG_DIR`, then `HOME`), including a worker's scoped home.
- [ ] The user-visible notice appears before any of the agent's own error
      output for that turn.
- [ ] Executor unit tests, clippy and formatting pass.

## Clarifications

No answers came with the command, so both questions are resolved from
recorded knowledge and the constitution.

- **Reset point with a missing transcript → start fresh.** A reset (edit or
  retry from an earlier message) can't be honoured without the transcript.
  Failing would leave the workspace stuck again, which is the defect being
  fixed. The fresh turn runs the user's message with the same agent and
  user notices (FR-2, FR-4, FR-5), so nothing is hidden. (Constitution
  XLVI.)
- **No summary seeding.** `vk/6026-no-conversation` records that VK must
  not fabricate Claude-private history from normalized logs, and that
  seeding a new conversation with a summary is "a separate recovery
  choice". The agent rebuilds context from the workspace's files and git
  history (FR-4). Seeding stays out of scope.

## Open Questions

None.
