# Feature Specification: Keep worker control metadata out of the agent chat

**Feature dir**: `specs/vk/5276-debug-unrecogniz/`
**Status**: Clarified

## Summary
When a user stops a running agent turn (Stop button, sending a follow-up that
interrupts, restart, affinity migration), the chat shows a system line:

    Unrecognized JSON message: {"cancellation_phase":"requested"}

This line is not from the agent. The cluster worker that runs the agent records
its own lifecycle metadata (cancellation phases, output-stream read failures,
worker launch failures) in the same ordered event journal as the agent's output.
The coordinator copies any such metadata that is not a Vibe Kanban log message
into the execution's **agent stdout**. The agent's log parser then sees a JSON
line it does not recognise and renders it as "Unrecognized JSON message". The
line appears more often now because every execution runs on a worker and
stop/interrupt flows are used more often. The noise makes users think the agent
or protocol is broken. It also hides the useful failure metadata
(`worker_error`, `stream_error`) behind a confusing label.

## User Stories
- As a user who stops an agent turn, I want the chat to show only the agent's
  output and the normal "stopped" outcome, not raw JSON bookkeeping.
- As a user whose execution failed inside the worker (for example, the agent
  process could not be launched or its output could not be read), I want a
  readable error that says what failed, not "Unrecognized JSON message".
- As a maintainer, I want worker metadata consumed by the code that owns it, so
  that vendor log parsers only ever see what the agent wrote.

## Functional Requirements
- FR-1: Worker cancellation-phase metadata (requested, terminating process
  group, killing process group) MUST NOT appear in an execution's chat or in
  its agent output stream.
- FR-2: Worker failure metadata (a worker-side launch/run failure reason and an
  output-stream read error) MUST be shown to the user as a readable Vibe Kanban
  error line that names what failed and keeps the original reason text verbatim
  (constitution XI).
- FR-3: Worker events that are already Vibe Kanban log messages MUST keep their
  current behaviour (delivered as-is, including final-assistant-state tracking).
- FR-4: Worker metadata of a kind the coordinator does not recognise MUST NOT be
  injected as agent output. It MUST stay visible as a Vibe Kanban diagnostic
  that preserves the original text, so a future worker/coordinator version skew
  cannot silently lose information.
- FR-5: Agent stdout and stderr bytes from the worker MUST be delivered exactly as
  today.
- FR-6: The change MUST work for every executor type, not only Claude, because
  the leak happens before any executor-specific parsing.

## Out of Scope
- Rewriting execution logs that were already persisted with the leaked line.
  Historical executions are immutable evidence and keep what they recorded.
- Changing the worker's event journal format or the cluster protocol.
- Changing how any executor parser treats genuinely unknown agent events.
- Showing cancellation progress in the UI. The terminal "killed" status already
  conveys the outcome.

## Acceptance Criteria
- [ ] Stopping a running turn no longer adds an "Unrecognized JSON message:
      {"cancellation_phase":…}" entry to the chat. This is covered by a
      regression test that feeds `{"cancellation_phase":"requested"}` through the
      coordinator's worker-event handling and asserts that nothing reaches the
      stdout stream.
- [ ] `{"worker_error":"<reason>"}` and `{"stream_error":"<reason>"}` each
      produce one stderr log line that contains `<reason>` verbatim, and nothing
      on stdout. This is covered by tests.
- [ ] A `Structured` payload that is a valid `LogMsg` is still pushed unchanged.
      This is covered by a test.
- [ ] An unrecognised `Structured` payload produces a stderr diagnostic that
      contains the original JSON, and nothing on stdout. This is covered by a
      test.
- [ ] `cargo test -p local-deployment` and the workspace checks pass.

## Open Questions
None. Resolved in `clarifications.md`:
- Historical executions keep what they recorded (no read-side cleanup).
- Cancellation phases are dropped from the chat and traced at `debug` on the
  coordinator.
- Worker error wording: `Worker error: <reason>`, `Worker output stream error:
  <reason>`, and `Unrecognized worker event: <json>`, all on stderr.
