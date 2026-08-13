# Implementation Plan: Resource-Aware Chat Loading

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, Tokio, Axum WebSockets, SQLx/SQLite, and executor-owned
  normalized-log processors.
- Primary seam:
  `crates/services/src/services/container.rs::stream_normalized_logs`.
- Durable cache:
  `crates/services/src/services/normalized_log_cache.rs`, stored atomically
  beside raw process logs via paths from `crates/utils/src/execution_logs.rs`.
- Request boundary:
  `crates/server/src/routes/execution_processes.rs::stream_normalized_logs_ws`.
- Existing resource controls: one process-wide historical-normalization
  semaphore, a 2,000-normalizable-message legacy cap, and abort-on-stream-drop.
- Deployment: one local Vibe Kanban coordinator serves workspace chat; cluster
  workers own dispatched live agents but are not generic read-job executors.
- No new dependency, database migration, generated type, frontend contract, or
  homelab module change is expected.

## Architecture & Approach

### 1. Add execution-keyed single-flight ownership

Add a narrow coordinator in `crates/services/src/services/container.rs` (or a
small sibling module if tests make that cleaner) mapping execution UUIDs to
weakly retained async mutexes. The map lock is held only while resolving the
per-execution cell; it is never held across database, filesystem, normalizer,
or WebSocket work.

After the optimistic durable-cache read misses, acquire the execution lock and
recheck the cache. The first reader remains leader and enters the existing
historical-normalization capacity queue. A concurrent reader waits on the same
execution lock, then observes and replays the completed sidecar. Different
executions remain governed by the global capacity semaphore.

Retain the execution guard in the returned stream lifetime alongside the
capacity permit and normalizer abort handles. This deliberately couples
leadership to complete consumption: atomic materialization happens at stream
end, and dropping the leader stream aborts its work and unlocks the execution
for a waiting reader to retry. Weak cells prevent the key map from growing with
historical execution count.

### 2. Preserve cache-hit and cancellation fast paths

Keep the first cache lookup before both single-flight and capacity acquisition.
Keep the second lookup after single-flight acquisition but before the capacity
semaphore and worktree recreation. Extract the cache replay construction to one
helper so both reads have identical validation and patch behavior.

The request route already races initial stream acquisition against inbound
socket closure, and the returned stream lifetime aborts normalizer tasks on
drop. Waiting readers are therefore independently cancellable; leader
disconnect releases ownership; remaining waiters retry in order; and no partial
sidecar becomes readable.

### 3. Add structured resource diagnostics

Trace, with execution ID and safe counts/durations:

- optimistic cache hit;
- cache miss and single-flight wait start;
- leader acquisition or cache hit after wait;
- global-capacity queue duration;
- normalization start with total/retained/dropped message counts;
- materialization completion and entry count;
- stream drop/cancellation before completion;
- materialization/normalization failure.

Do not log raw messages, normalized entries, prompts, or patch contents. Reuse
the existing node/process metrics UI to observe CPU; observability remains
read-only and does not choose placement.

### 4. Test the coordination contract

Factor the single-flight registry enough to run deterministic Tokio tests with
synthetic execution IDs. Prove:

- two concurrent misses have one leader;
- the second reader waits and becomes a completed-result reader after the
  leader publishes success;
- distinct execution IDs do not share the key lock;
- canceling a waiter does not cancel the leader;
- dropping/failing the leader releases ownership for retry;
- unused key cells are reclaimable.

Extend normalized-cache/container tests where practical to prove cache-hit
bypass and atomic partial-file refusal. Avoid a timing-only WebSocket test when
the service ownership seam can provide deterministic ordering.

### 5. Measure before distributing

Use structured durations and a representative long-log fixture or focused
benchmark to compare a cold leader, a concurrent waiter, and a warm cache hit.
Do not repurpose `ExecutionWorkerJob`: it models an affinity-bound live agent,
not a reconstruction read. Record a future protocol need only if clarified p95
latency/CPU gates remain exceeded after this change.

## Data Model

See `./data-model.md`. No persistent schema change is planned.

## Contracts

See `./contracts/historical-materialization.md`. The external normalized-log
WebSocket contract is unchanged.

## Research Notes

See `./research.md`. No external dependency is introduced.

## Constitution Check

- II: deterministic concurrency, retry, cache-integrity, and cancellation tests
  validate the contract.
- III and VI: the change composes the shipped bounded replay and durable cache
  instead of introducing paging or distributed job infrastructure.
- IX: existing vendor normalization and patch ordering remain authoritative.
- XII and XXVIII: one execution-keyed owner performs expensive work; locks do
  not span unrelated operations, and cancellation/retry outcomes are explicit.
- XVIII, XIX, and XXII: metrics do not schedule work and a read never migrates
  affinity.
- XXXI: cache hits bypass queues, misses single-flight, completion is atomic,
  state is bounded, and failures remain retryable.

No constitution deviation remains.

## Risks & Dependencies

- A guard stored inside a boxed stream must outlive every emitted item and drop
  promptly on WebSocket closure; tests must exercise both completion and early
  drop.
- If sidecar persistence fails after a leader successfully streams results, the
  next waiter must reconstruct rather than assume success. This is correct but
  can repeat work during disk failure; the diagnostic must make it actionable.
- An empty normalized transcript currently is not materialized. If valid empty
  completed processes reach this route, waiters could repeat cheap work; do not
  change the cache format without proving this is material to the CPU symptom.
- Process-local coordination assumes the deployed single coordinator. Atomic
  cache validation remains safe across restarts, but it does not exclude two
  separately configured servers sharing one data directory.
