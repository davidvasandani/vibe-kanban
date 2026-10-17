# Worker journal vs. agent output stream

A cluster worker records **two kinds of data** in one ordered execution journal:

- the agent's raw bytes (`ExecutionEventPayload::Stdout` / `Stderr`), and
- Vibe Kanban's own metadata (`ExecutionEventPayload::Structured { json }`),
  currently `{"cancellation_phase": …}` (`crates/worker/src/cancellation.rs`),
  `{"stream_error": …}`, and `{"worker_error": …}` (`crates/worker/src/execution.rs`).

The coordinator's worker-event poll loop in `crates/local-deployment/src/container.rs`
is the **only** place where these are separated. Everything it pushes as stdout is
parsed by the executor's vendor log normalizer. Any JSON line that the normalizer does
not know becomes a chat entry. For Claude this is `ClaudeJson::Unknown` →
"Unrecognized JSON message: …".

## Gotcha: the fallback that leaked
Before `vk/5276-debug-unrecogniz`, the loop tried to parse `Structured` as `LogMsg`
and pushed **anything else to agent stdout**. None of the worker's `Structured`
producers emits a `LogMsg`, so every stop or interrupt of a worker-run turn put
`{"cancellation_phase":"requested"}` into the chat. This affected every executor
(Claude, Codex, and others), not just Claude. It surfaced "more often" as more flows
started to route through worker cancellation: Stop, rejected tool calls,
queued follow-ups, restarts, and affinity migration.

## Rule (constitution IX, 0.37.0)
`classify_worker_structured` routes `Structured` payloads as follows:

- `LogMsg` → passed through unchanged;
- `cancellation_phase` → `tracing::debug!` only (the following `Killed` event
  carries the outcome);
- `worker_error` / `stream_error` → a readable `LogMsg::Stderr` diagnostic with
  the reason verbatim;
- anything else → `Unrecognized worker event: <json>` on stderr.

Only worker `Stdout` bytes and `LogMsg::Stdout` may write agent stdout. When
adding a new `Structured` kind in the worker, add an arm to the classifier. An
unmatched kind stays visible as a stderr diagnostic, but it will never be parsed
as agent output.

## Rejected alternative
Teaching each vendor parser to ignore product metadata. That approach needs one
change per executor, couples vendor parsing to VK internals, and would hide an
identically shaped line that a real agent emitted.

## Not fixed
Persisted logs written before the fix still contain the leaked line. History
is immutable, and no read-side filter was added.

## Contributed by

- vk/5276-debug-unrecogniz
