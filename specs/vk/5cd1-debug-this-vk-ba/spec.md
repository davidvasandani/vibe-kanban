# Feature Specification: reliable Claude background-Bash denial

**Feature dir**: `specs/vk/5cd1-debug-this-vk-ba/`
**Status**: Draft

## Summary

Vibe Kanban must reliably reject Claude's in-turn background Bash requests and
direct the agent to durable VK pollers. Today the denial hook can lose its
control-protocol response when the structured input stream closes, producing a
large `Error in hook callback DENY_BACKGROUND_BASH_CALLBACK_ID` diagnostic
instead of a normal actionable denial.

## User stories

- As a Vibe Kanban user, I want Claude to receive a clear denial when it tries
  to launch an unsupported background command so the agent can continue using
  the supported poller workflow.
- As an operator, I want hook transport failures to be distinguishable from
  successful execution so tasks do not appear healthy after losing a required
  permission response.
- As a maintainer, I want lifecycle-ordering regression coverage so future
  result, cancellation, or EOF changes cannot silently close the control stream
  with a callback pending.

## Functional requirements

- **FR-1:** An explicit Claude `Bash` request with
  `run_in_background: true` must be denied in every permission mode.
- **FR-2:** The denial shown to Claude must explain that in-turn background
  processes do not survive and must name `spawn_poller` as the supported
  durable replacement.
- **FR-3:** Foreground Bash and ambiguous or malformed background flags must
  retain their existing permission behavior.
- **FR-4:** Once Vibe Kanban accepts a Claude control request, execution
  teardown must not close the response stream before the request has been
  answered or explicitly failed.
- **FR-5:** Terminal results, cancellation, timeouts, and stdout closure must
  terminate cleanly without an indefinite wait and without discarding accepted
  callback work.
- **FR-6:** A required control-response write failure must be visible as an
  executor/protocol failure and must not be reported as an ordinary successful
  denial.
- **FR-7:** Existing late Stop-hook handling and zero-turn resume-artifact
  handling must continue to work.
- **FR-8:** The fix must remain confined to the Vibe Kanban service.

## Out of scope

- Enabling Claude's native background Bash, Monitor, TaskOutput, TaskStop, or
  Cron tools.
- Changing VK poller scheduling, persistence, helper limits, or UI.
- Modifying Claude, any MCP server, or any other hosted service.
- A dependency upgrade with no application-owned lifecycle correction.

## Acceptance criteria

- [ ] A transport-level regression test drives the failing control-request
      ordering and observes the complete deny response before input teardown.
- [ ] The test would fail against the pre-fix control-stream lifecycle.
- [ ] No test or live reproduction emits `Stream closed` for
      `DENY_BACKGROUND_BASH_CALLBACK_ID`.
- [ ] The returned reason includes `spawn_poller`.
- [ ] Existing auto, supervised, and plan-mode hook tests pass.
- [ ] Foreground Bash behavior remains unchanged.
- [ ] Cancellation and clean terminal completion remain bounded.
- [ ] Focused executor checks and formatting pass.

## Clarified decisions

- The failing boundary is Vibe Kanban's fixed post-result deadline. Claude can
  emit post-result protocol activity while SDK-managed/background work drains;
  measuring 500 ms from the result closes stdin even when that activity proves
  the control channel is still active.
- The protocol loop will use generic async reader/writer bounds internally so a
  Tokio duplex transport can test timing and response delivery without a real
  Claude process. The production API will continue accepting
  `ChildStdin`/`ChildStdout`.
- A bounded post-result grace remains necessary because structured-input Claude
  waits for EOF before exiting. The grace is an **idle/quiescence** deadline:
  every post-result line resets it, and an accepted control request is answered
  inline before the next deadline check. The existing duration remains the
  initial bound unless pinned-artifact reproduction demonstrates it is too
  short even in complete silence.

## Open questions

None.
