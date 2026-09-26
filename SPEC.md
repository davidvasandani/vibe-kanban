# SPEC — Keep worker control metadata out of the agent chat (vk/5276)

Feature artifacts: `specs/vk/5276-debug-unrecogniz/` (spec, clarifications,
plan, research, contract, tasks).

## Problem
Stopping or interrupting a worker-run turn adds this system line to the chat:
`Unrecognized JSON message: {"cancellation_phase":"requested"}`. It was
reproduced live in this task's own session when a tool call was rejected.

## Root cause
- The worker (`crates/worker`) appends product-owned metadata to the execution
  journal as `ExecutionEventPayload::Structured { json }`:
  - `{"cancellation_phase": …}`: `cancellation.rs` `structured_phase`
  - `{"stream_error": …}`: `execution.rs` `stream_output`
  - `{"worker_error": …}`: `execution.rs` `finish_failed`
- The coordinator's worker-event poll loop (`crates/local-deployment/src/container.rs`)
  accepted `Structured` payloads that parse as `LogMsg`. It pushed **everything
  else to agent stdout**.
- The executor's log normalizer then parsed that stdout line as an agent event
  and rendered it through its unknown-event branch (Claude: `ClaudeJson::Unknown`).

## Behaviour after the change
| Structured payload | Result |
|---|---|
| valid `LogMsg` | unchanged (pushed, final-assistant tracking) |
| `cancellation_phase` | `tracing::debug!` on the coordinator only; nothing in the chat |
| `worker_error` | stderr: `Worker error: <reason>` |
| `stream_error` | stderr: `Worker output stream error: <reason>` |
| anything else | stderr: `Unrecognized worker event: <json>` |

After the change, agent stdout is written only by worker `Stdout` bytes and `LogMsg::Stdout`.

## Non-goals
Historical logs are not rewritten. The protocol and the vendor parsers are not changed.

## Acceptance
Unit tests in `container::worker_event_tests` cover every row above.
`cargo test -p local-deployment` and `cargo clippy -p local-deployment --all-targets` are clean.
