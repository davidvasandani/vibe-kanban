# Feature Specification: A turn that never started can always be stopped

**Feature dir**: `specs/vk/e524-debug-stuck-sess/`
**Status**: Clarified

## Summary
A session can show a running turn that no agent is working on. This happened
to the Ohana VAS-1059 workspace: a "Continue" follow-up was recorded as
running, but the request was abandoned before the turn was handed to an
execution server. No agent ever started. The Stop button then did nothing,
a hard refresh did not help, and the session could not take a new turn,
because Vibe Kanban still believed one was running. Only a coordinator
restart would have cleared it. This feature makes such a turn stoppable,
stops it from arising when a request is abandoned, and makes a failed stop
visible to operators.

## User Stories
- As a user, I want Stop to end a turn that shows as running even when no
  agent ever started, so that I can send the next message instead of being
  locked out of my session.
- As a user, I want a turn whose start was abandoned to end on its own with
  a clear failure, so that I never see a turn that runs forever with no
  output.
- As an operator, I want a failed stop to appear in the coordinator log, so
  that I can diagnose a stuck session without reproducing it.
- As a user whose agent was interrupted by a server restart, I want my
  uncommitted work to be preserved exactly as it is today, so that this
  change costs me nothing.

## Functional Requirements
- FR-1: Stopping a turn that is recorded as running but has no evidence of
  ever launching (no process, no execution-server job) MUST record the
  outcome the caller requested and report success.
- FR-2: After FR-1, the session MUST accept a new turn without a restart.
  The exception is a start that is hung rather than abandoned (FR-5): it
  still holds the session's start lock, so a new turn waits until it
  returns.
- FR-3: If the request that starts a turn is abandoned before the turn is
  handed to a local process or an execution server, the turn MUST be
  recorded as failed, with a message in its log saying the start was
  abandoned.
- FR-4: A turn that is genuinely still starting MUST NOT be terminated by
  FR-1. Stop reports that it is still starting.
- FR-5: A start that has not handed its turn off within 120 seconds is
  treated as abandoned for FR-1.
- FR-6: A turn that did launch (it has a process or an execution-server
  job) MUST keep its current stop and recovery behaviour, including
  preservation of uncommitted work at the next restart.
- FR-7: A terminal outcome that another part of the system has already
  recorded MUST NOT be overwritten by FR-1 or FR-3.
- FR-8: A stop that fails MUST be logged at warning level with the turn and
  workspace identifiers.
- FR-9: A turn ended by FR-1 or FR-3 is never recorded as completed.

## Out of Scope
- Running turn starts independently of the request that began them.
- A background sweep for running turns that have no owner.
- A time limit on the work done before a turn is handed off.
- Changes to the chat interface.
- Editing the stuck turn's record by hand.

## Acceptance Criteria
- [ ] Stopping a running turn with no launch evidence, when no start is in
      progress, leaves it terminal with the requested outcome, and the stop
      reports success.
- [ ] Stopping a running turn that has a recorded process group but no live
      process still reports failure and leaves the turn running, as today.
- [ ] Stopping a turn whose start began less than the bound ago reports
      that it is still starting and leaves it running.
- [ ] Stopping a turn whose start began longer than the bound ago leaves
      it terminal.
- [ ] Abandoning a start before handoff leaves the turn failed; abandoning
      it after an execution-server job exists leaves the turn untouched.
- [ ] A turn that is already terminal is unchanged by a late stop or a late
      abandoned-start cleanup.
- [ ] A failed workspace stop writes a warning to the coordinator log.
- [ ] After deployment, execution `5cc7e118-c02b-4c33-ad7e-6a864fb53ad7` is
      terminal and its session accepts a new turn.

## Clarifications

Resolved on 2026-11-17 from the incident evidence and the existing code.
No answers were supplied to the clarify stage, so each records its
reasoning.

- **Outcome of a stopped never-launched turn.** It takes the outcome the
  caller asked for: killed for a user stop, interrupted for a shutdown.
  Every other stop already reports this way, and the chat already renders
  both. Failed is reserved for FR-3, where nobody asked for the stop.
- **Idle bound before a start counts as abandoned.** 120 seconds. The work
  before handoff is database lookups, secret resolution and a
  configuration snapshot, which normally finish in a few seconds. The
  bound only matters when a user presses Stop on a start that is hung
  rather than dropped; a dropped start needs no bound (FR-3).
- **The live stuck turn.** Left for the deployment restart. On start-up the
  coordinator already marks a running turn with no execution-server job as
  interrupted, so deploying this fix clears it with no manual database
  edit. This is a default, not a requirement: clearing it sooner needs
  either a coordinator restart or a direct database write, and both need
  the owner's go-ahead.

## Open Questions
None.
