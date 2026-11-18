# Feature Specification: Automatic exponential backoff for model-capacity errors

**Feature dir**: `specs/vk/e273-automatic-expone/`
**Status**: Draft

## Summary
When a coding-agent turn ends with the provider error "Selected model is at
capacity. Please try a different model.", the work is unfinished but the
session stops and waits for someone to type "continue". Capacity outages last
minutes to hours and usually hit several workspaces at once, so unattended
pipelines stall overnight. Vibe Kanban will resume such a session by itself
after a delay that doubles with each consecutive failure, and stop trying after
a fixed number of attempts.

## User Stories
- As an operator running unattended pipelines, I want a session that hit a
  capacity error to continue on its own so that an upstream outage costs
  minutes, not a night.
- As an operator, I want to see that a retry is pending, when it will fire and
  which attempt it is, and to cancel it, so that automatic behavior never
  surprises me.
- As a user who continues the session myself, I want the pending retry to step
  aside so that I do not get a second, unwanted "continue" turn.
- As an operator, I want to turn the behavior off in Settings.

## Functional Requirements
- FR-1 (Detect): A finalized coding-agent turn, recorded as completed or
  failed, whose log ends with a capacity error schedules an automatic resume.
  "Ends with" means the error is an error entry and no agent message or tool
  use follows it.
- FR-2 (Evidence only): The same words in a user message, an agent message or
  tool output never trigger a resume. A log that cannot be read triggers
  nothing. User stops, server restarts and indeterminate exits trigger
  nothing.
- FR-3 (Backoff): Attempt n waits the base delay doubled n−1 times, capped at
  a maximum, plus up to 20 % random jitter. Defaults: 60 s base, 3600 s
  maximum, 10 attempts.
- FR-4 (Chain): A capacity failure of a turn started by retry n schedules
  retry n+1. Any other capacity failure starts again at 1. After the last
  attempt nothing more is scheduled and the give-up is logged.
- FR-5 (Resume): At the deadline the session receives a system-authored
  message saying the previous turn hit a capacity error and asking it to
  continue. The session's agent, model and settings are unchanged.
- FR-6 (Yield): A pending retry is cancelled with a recorded reason, not
  delivered, when the session has started any newer turn, or when the
  workspace is archived, deleted or has lost its worktree. A message queued
  ahead of the retry runs first and thereby cancels it.
- FR-7 (Idempotent): A failed turn produces at most one retry, including under
  duplicate events and restarts.
- FR-8 (Independent budget): Retries neither count against nor are blocked by
  the agent's own wake-up limits.
- FR-9 (No unarchive): Scheduling a retry never changes a workspace's archive
  state.
- FR-10 (Durable): A scheduled retry survives a server restart and fires late
  rather than never.
- FR-11 (Visible): The retry appears in the Processes drawer with its
  countdown and attempt number, and can be cancelled there.
- FR-12 (Configurable): Enabled, base delay, maximum delay and attempt count
  are settings. The feature is enabled by default. Settings → General has a
  toggle. A change applies to the next failure without a restart.
- FR-13 (No remediation noise): Auto error remediation does not file an issue
  for a turn that failed with a capacity error.

## Out of Scope
- Switching to another model.
- Other transient errors such as rate limits or "overloaded" responses.
- Any change outside Vibe Kanban.

## Acceptance Criteria
- [ ] A turn whose last entry is the capacity error gets one pending retry due
      in 60–72 s; a second identical event adds none.
- [ ] Ten consecutive capacity failures produce delays of 60, 120, 240, 480,
      960, 1920, 3600, 3600, 3600, 3600 s before jitter; the eleventh schedules
      nothing.
- [ ] A retry resumes a capacity turn recorded as failed, while an ordinary
      agent wake-up still refuses a failed turn.
- [ ] If a newer turn exists when the retry is due, the retry ends cancelled
      and no turn starts.
- [ ] A session with eight pending agent wake-ups still gets its retry, and a
      pending retry does not reduce the agent's eight.
- [ ] An archived workspace gets no retry and stays archived.
- [ ] With the setting off, no retry is scheduled.
- [ ] The Processes drawer row reads "Capacity retry (attempt N)".
- [ ] A failed capacity turn files no `Auto-fix:` issue.

## Clarifications
No answers were supplied to the clarify stage, so each question is resolved
from the task text, the code and the constitution. Each is an assumption the
operator can overrule.

- **Script or server-owned schedule?** Server-owned schedule. The task's goal
  is that the session resumes automatically. A workspace shell script dies
  with its process or the server, cannot take the session admission lock, and
  could start a turn beside a human's. The durable wake-up ledger already
  provides the timer, restart survival, admission and a cancel control
  (constitution LX, LVII).
- **Enabled by default?** Yes. The task says a capacity error "should
  automatically" be retried. Constitution LX permits this because the retry
  only continues a session a human started and is bounded per chain; XLI's
  opt-in rule covers work that files issues or starts workspaces. Startup
  resume stays opt-in because it can start many agents at once at boot.
- **Try a different model?** No. Changing the model changes cost and behavior
  without consent. The retry keeps the session's configuration (FR-5).

## Open Questions
None remaining.
