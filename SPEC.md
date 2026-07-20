# Technical Spec: Dispatch Queued Messages When Cleanup Is Skipped

## Problem

When a successful coding-agent run makes no repository changes, the exit monitor
skips the configured cleanup action, manually finalizes the task, and sets
`already_finalized`. That flag bypasses the later finalization block containing
the only normal queued-message consumer. A message submitted during the run
therefore remains in the in-memory queue indefinitely. The screenshot's "0 files
changed" state matches this branch exactly.

## Required Change

- Before manually finalizing the no-changes branch, claim any queued follow-up.
- Delete its draft scratch and start it through the existing queued follow-up
  execution helper.
- Finalize normally when no message exists or follow-up start fails.
- Preserve the existing cleanup-skip behavior and all other queue consumers.
- Add focused regression coverage that fixes the decision contract: skipped
  cleanup dispatches a queued follow-up, while the empty case finalizes.

## Non-goals

- Changing queue persistence, API types, frontend behavior, or replacement and
  cancellation semantics.
- Redesigning general completion/submission concurrency.

## Acceptance Criteria

- A queued message present when a successful coding run produces no changes is
  consumed and started without cancellation/resubmission.
- A no-change run with no queued message still finalizes and skips cleanup.
- Failure to start the claimed follow-up falls back to task finalization.
- Existing finalization and queued-message tests remain green.
