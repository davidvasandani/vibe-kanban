# Implementation Plan: Keep worker control metadata out of the agent chat

**Spec**: `./spec.md`
**Status**: Planned

## Technical Context
- Rust workspace. The fix is confined to `crates/local-deployment` (the
  coordinator side of remote/worker execution).
- Producer (unchanged): `crates/worker` appends
  `ExecutionEventPayload::Structured { json }` (`crates/cluster-protocol/src/lib.rs:344`)
  for three product-owned payloads:
  - `{"cancellation_phase": <CancellationPhase>}`: `crates/worker/src/cancellation.rs:194`
    (`structured_phase`), emitted on the `requested`, `terminating_process_group`, and
    `killing_process_group` transitions.
  - `{"stream_error": "<io error>"}`: `crates/worker/src/execution.rs:1493`
    (`stream_output`).
  - `{"worker_error": "<reason>"}`: `crates/worker/src/execution.rs:1570`
    (`finish_failed`).
- Consumer (the defect): the worker event poll loop in
  `crates/local-deployment/src/container.rs:2859`. It tries
  `serde_json::from_str::<LogMsg>(&json)`. If that fails, it calls
  `store.push_stdout(format!("{json}\n"))`, which writes the product metadata into
  the agent's stdout. The executor's log normalizer (for example,
  `crates/executors/src/executors/claude.rs:2609`, `ClaudeJson::Unknown`) then
  renders it as `Unrecognized JSON message: …`.

## Architecture & Approach
1. Add a small pure classifier next to `push_worker_bytes` in
   `crates/local-deployment/src/container.rs`:

   ```rust
   enum WorkerStructuredEvent {
       Log(LogMsg),
       CancellationPhase(String),
       Diagnostic(String),
   }
   fn classify_worker_structured(json: &str) -> WorkerStructuredEvent
   ```
   - The classifier tries `LogMsg` first, which preserves FR-3.
   - It then parses the JSON as a single-key object:
     - `cancellation_phase` → `CancellationPhase(value)` (FR-1).
     - `worker_error` with a string value → `Diagnostic("Worker error: <reason>")` (FR-2).
     - `stream_error` with a string value → `Diagnostic("Worker output stream error: <reason>")` (FR-2).
   - Anything else → `Diagnostic("Unrecognized worker event: <json>")` (FR-4).
2. Replace the `Structured` arm in the poll loop:
   - `Log(message)` → the existing final-assistant-state tracking plus `store.push(message)`.
   - `CancellationPhase(phase)` → `tracing::debug!(%execution_id, phase, …)`
     only. Nothing is pushed.
   - `Diagnostic(text)` → `store.push(LogMsg::Stderr(text))`. Stderr is the
     existing channel for coordinator-authored diagnostics in this loop, for example
     "Worker reported an indeterminate execution" and the replay-gap line.
3. The Stdout and Stderr arms are unchanged (FR-5). The change applies before any
   executor-specific parsing, so it covers every executor (FR-6).

## Data Model
No persistent data changes. No data-model document is needed.

## Contracts
See `./contracts/worker-structured-events.md` for the coordinator's
handling contract for `Structured` worker events.

## Research Notes
See `./research.md`.

## Constitution Check
- **IX (extended in 0.37.0)**: product metadata no longer enters the agent's
  output stream, and the fix sits at the point where it leaked. Vendor parsers are
  not changed. ✅
- **XI (diagnostics are evidence)**: worker error reasons and unrecognised
  payloads are kept verbatim as stderr diagnostics, not dropped. ✅
- **XVIII (evidence-backed execution)**: terminal state still comes from the
  worker's Completed, Killed, or Indeterminate events. Cancellation phases were never
  lifecycle authority and are only traced. ✅
- **II (test the contract)**: unit tests cover each classifier outcome, and the
  tests sit with the existing `push_worker_bytes` tests. ✅
- **III/VI (small, reuse)**: the change is one function and one match arm, with no
  protocol change and no new dependency. ✅

## Risks & Dependencies
- A future worker could emit a new `Structured` kind before the coordinator knows
  it. That kind now becomes a stderr diagnostic instead of agent stdout. This is
  intended (FR-4).
- Old coordinator with new worker, or the reverse: there is no protocol change, so
  the risk is none.
- Executions persisted before this fix keep the leaked line (Out of Scope, Q1).
