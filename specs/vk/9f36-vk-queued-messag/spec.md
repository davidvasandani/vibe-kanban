# Feature Specification: Queued Follow-up After No-change Run

**Feature dir**: `specs/vk/9f36-vk-queued-messag/`
**Status**: Clarified (no open questions)
**Task**: `vk/9f36-vk-queued-messag`

## Summary

A queued follow-up currently hangs when the preceding successful coding-agent
run makes no repository changes. That path skips cleanup and manually finalizes,
bypassing the later block that consumes queued messages. The feature ensures the
pending follow-up is dispatched before that early finalization completes.

## Why

The composer promises a queued message will execute when the current run
finishes. In the reported `0 files changed` case, the run finishes but the queue
stays occupied until the user cancels and resubmits. Delivery should not depend
on whether the preceding turn happened to modify files.

## User Stories

- As a user who queues a follow-up during a read-only/no-change run, I want it to
  start automatically when that run ends.
- As a user with no queued follow-up, I want no-change runs to retain their fast
  cleanup-skip finalization behavior.

## Functional Requirements

- FR-1: When a successful coding-agent run skips cleanup because it produced no
  changes, the system MUST check for and claim a queued follow-up before manual
  finalization.
- FR-2: A claimed message MUST start through the existing queued-follow-up path,
  preserving content, executor configuration, session continuity, working
  directory, scratch cleanup, and action construction.
- FR-3: If no queued message exists, the task MUST finalize exactly as before.
- FR-4: If starting the claimed follow-up fails, the task MUST fall back to
  finalization and log the failure exactly as the normal queue consumer does.
- FR-5: Existing cleanup execution when changes exist, normal finalization queue
  consumption, failed/killed/interrupted discard behavior, and parallel setup
  handling MUST remain unchanged.
- FR-6: The change MUST NOT alter queue HTTP or frontend contracts.

## Out of Scope

- Queue persistence across restarts or multiple queued messages.
- General completion/submission race redesign.
- Frontend UI changes.
- Changes to failed, killed, or interrupted execution policy.

## Acceptance Criteria

- [ ] With a queued message and a successful no-change coding run, cleanup is
      skipped and the queued follow-up is started once.
- [ ] With no queued message, the same run manually finalizes as before.
- [ ] A follow-up start failure falls back to finalization.
- [ ] Existing changed-run, normal finalization, and setup queue paths remain
      green.
- [ ] Focused tests, relevant checks, and formatting pass.

## Clarifications

- The screenshot's `0 files changed` indicator and the code's
  `already_finalized` guard identify the concrete failure; implementation is
  scoped to that branch rather than a speculative API/state-machine redesign.
- Scratch deletion and execution start are shared with the normal consumer via a
  local helper so both paths keep the same behavior.
