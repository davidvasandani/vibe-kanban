# Feature Specification: A worker restart never leaves a stopped job looking Running

**Feature dir**: `specs/vk/9c15-stopped-job-look/`
**Status**: Draft
**Task**: `vk/9c15-stopped-job-look` (technical detail in the repo-root `SPEC.md`)

## Summary

When a cluster worker restarts without first draining, the agent it was
running dies. The worker records the job as **interrupted** when it comes
back. Vibe Kanban never notices that record. The chat keeps showing the run
as active: the Stop button spins and the last tool call stays in its loading
state. This lasts until someone presses Stop, which only then reveals "This
run was interrupted by a vibe-kanban restart" with **Resume**. In the
reported incident the stale state lasted about 25 minutes.

The cause is that the restarted worker numbers its recovered "interrupted"
record lower than the events the coordinator has already read. The
coordinator keeps waiting for events newer than its position, which never
come. This feature makes the coordinator recognise the regression and
resolve the run from the worker's own record. A run the worker reports
interrupted shows as interrupted with Resume, and a run with no trustworthy
record is shown with an unknown outcome. Neither stays "running".

## User Stories

- As a user whose task was running on a worker that restarted, I want the
  chat to show "interrupted" with **Resume** on its own, so that I do not
  have to press Stop to find out the run already ended.
- As a user, I want a run whose outcome cannot be established to show as
  ended with an unknown outcome, never as running.
- As an operator reading a task after such a restart, I want the chat to say
  that the worker restarted and that some output may be missing.
- As a user of a healthy run, I want nothing to change: live output,
  approvals and coordinator-restart handoff keep working.

## Functional Requirements

- **FR-1**: When a worker reports that its latest event for an execution is
  older than the latest event the coordinator has already consumed, the
  coordinator treats this as a lost event stream, not as "no new events".
- **FR-2**: On a lost stream, the coordinator checks the worker's job list.
  It ends the run with the worker's recorded outcome only when the record
  matches this exact dispatch (same worker, execution, job and request) and
  belongs to the worker's current event stream. An interrupted record makes
  the run show as interrupted, with Resume available.
- **FR-3**: Ending a run this way goes through the same completion steps as
  any other worker-reported outcome. That covers the follow-up action, the
  queued message, the notification and saving the work. It is not a separate
  shortcut.
- **FR-4**: The run is marked as having incomplete output, and the chat shows
  one notice that the worker restarted and later output may be missing.
- **FR-5**: If the worker's job list is readable but has no matching record,
  the run ends with an unknown outcome. If the job list or the stored job
  cannot be read, the coordinator keeps retrying and never guesses an
  outcome.
- **FR-6**: The fix applies whether the coordinator noticed live or after its
  own restart.

## Out of Scope

- Making the worker save its event position more often. It narrows the
  window, but a crash can still fall between delivering events and saving.
- Frontend changes. The UI already reflects the stored run state correctly.
- Repairing runs already ended manually, including the incident run.
- A worker that has lost its job records entirely.

## Acceptance Criteria

- With the incident's shape, the coordinator ends the run as interrupted on
  its first check after the worker restarts. In that shape the coordinator has
  read about 2,500 events and the worker's recovered "interrupted" record sits
  at event 6.
- A mismatched record (different job, request, worker or execution, a
  non-final state, contradictory evidence, or a record from another event
  stream) never ends the run with that record's outcome.
- All four final outcomes (completed, failed, killed, interrupted) are
  recognised.
- Healthy runs (worker latest event at or beyond the coordinator's position)
  are unaffected.
- Unit tests cover the regression check and the record matching. The Rust
  check, lint and format steps pass.

## Clarifications

- **Q: What counts as "belongs to the worker's current event stream" (FR-2)?**
  A: The record's latest event number equals the latest event number the
  worker just reported for this execution. A record from an older or newer
  stream would not match, so it cannot supply an outcome.
- **Q: What if the matching record still says the job is running after a
  regression?** A: The current worker cannot produce that, because recovery
  always records a final state. If it did happen, the stream and the record
  contradict each other, so the run ends with an unknown outcome (FR-5)
  rather than staying running.
- **Q: What if the user already pressed Stop while the run looked stuck?**
  A: A user's Stop keeps its existing precedence. The normal completion
  steps (FR-3) already skip overwriting a run the user stopped.

## Open Questions

- None. The incident evidence (worker logs, recovered job record) settles
  the cause and the expected UI.
