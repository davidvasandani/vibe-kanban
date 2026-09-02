# Research: Turn completion clears the running composer

## Root cause

The reported model label (`GPT-5.6 Sol · High`) identifies the Codex executor.
Codex is an app-server-style executor: one child process can remain alive after
the protocol turn has emitted final assistant output, while `turn/completed`
travels through a separate `ExecutorExitSignal`.

The prior stale-execution reconciliation added
`wait_for_unfinalized_output` in
`crates/local-deployment/src/container.rs`. After 45 seconds of quiet final
assistant output it checks `child_store`; if `try_wait() == None`, it treats the
live child as positive turn liveness, resets the timer, and repeats forever.

That rule is valid for natural-exit executors, where the process lifetime is
the turn lifetime. It is invalid for executors with an explicit exit signal:
the app-server child being alive proves only process liveness, not that the
protocol turn remains active. If Codex final output arrives but the
`turn/completed` signal is lost or its reader path stalls, the process row and
UI remain `running` indefinitely. The existing unit test
`positive_local_process_liveness_defers_reconciliation` codifies this
over-broad behavior without distinguishing executor lifecycle shape.

## Decision: qualify process liveness by lifecycle shape

Capture whether the spawned execution has an explicit executor exit signal
before moving that signal into the exit monitor future. Pass that fact into the
final-output reconciliation helper.

- Natural-exit executor (`exit_signal: None`): a live child is positive turn
  liveness and re-arms the timeout.
- Signal-driven executor (`exit_signal: Some`): a live child is not turn
  evidence. Quiet final output may reach the bounded timeout, after which the
  existing monitor kills the exact owned process group and records
  `indeterminate`.

Normal `turn/completed`, OS exit, stop, failure, and worker terminal evidence
still win immediately. Final output still never becomes `completed`.

## Alternatives rejected

### Clear the frontend when final text renders

Rejected because transcript output can precede finalization and cannot safely
remove Stop from a genuinely active turn. It would create a second local
running authority and violate Constitution Principle XXX.

### Treat every live process as active turn evidence

This is the current defect. App-server process lifetime and turn lifetime are
different axes.

### Kill every process 45 seconds after final output

Rejected because a natural-exit executor may still be doing work after an
intermediate-looking assistant entry, and the owned live child is positive
liveness for that lifecycle shape.

### Add a new Codex-only timer

Rejected because `SpawnedChild.exit_signal` already encodes the relevant
lifecycle distinction for Codex, OpenCode, and ACP. The container should use
the existing abstraction rather than duplicate executor identifiers.

## Dependencies

No new dependency, API, database migration, generated type, frontend change, or
homelab deployment change is required.
