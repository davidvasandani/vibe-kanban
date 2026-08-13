# Prior Knowledge: Resource-Aware Chat Loading

Task: `vk/6df4-loading-chat-pin`

The Vibe Kanban project knowledge base is not empty. These pages constrain the
specification and plan.

## `lazy-loading-normalized-conversation-history.md`

- Frontend row virtualization does not bound backend work: the browser still
  pays for normalized-log reconstruction, transfer, derivation, and retained
  state for every opened process.
- Persisted completed executions contain raw stdout/stderr rather than final
  normalized entries. A correct tail cannot be obtained by slicing raw JSONL or
  patch frames because normalizers retain lifecycle state and later operations
  depend on earlier entries.
- True bounded paging requires a durable final-entry materialization. Legacy
  histories must pay one full normalization, but that work should be observable,
  capacity-bounded, cancellable, and not tied to an interactive request.
- The shipped lazy-loading slice pages by completed process and caps a legacy
  normalization at the newest 2,000 normalizable messages. It does not page
  within one large process.

Current code advances the earlier design gate: commit `f31f58ae` added an
atomic sidecar materialized view for finished processes. A valid sidecar is
read before the historical-normalization semaphore or worktree recreation.
However, concurrent cache misses still independently normalize the same
execution, so the durable cache does not prevent a thundering herd on its first
read.

## `claude-log-normalization.md`

- Vendor normalizers emit ordered JSON Patch operations over an entries array;
  stdout and stderr producers share one atomic entry-index provider.
- Later patches can replace earlier entries, so the final conversation is a
  stateful reduction rather than a line-by-line projection.
- Normalizer reset and tool lifecycle behavior are correctness-sensitive.
  Resource changes should orchestrate existing normalizers, not introduce a
  second transcript parser.

## `clustered-workspace-execution.md`

- The coordinator owns SQLite authority, placement, user-facing state, and
  shared-worktree administration. A worker owns only the processes dispatched
  to its sticky workspace, including ordered output and cancellation.
- Worker dispatch identity is an execution job, with idempotency and affinity
  bound to that job. Reusing it for an unrelated read-only reconstruction job
  would broaden the protocol and recovery model.
- Shared storage is a verified capability, not merely a matching path. The
  cluster does have portable workspace storage, but every cross-node operation
  still needs mount-health and ownership evidence.
- An offline worker is indeterminate, not permission to silently move work.

Consequently, idle workers are not currently a safe transparent compute pool
for historical chat normalization. The first implementation should eliminate
duplicate work and keep normalization off the async request scheduler on the
serving/owning node. Distributed normalization is a later protocol feature only
if measurements show meaningful residual demand.

## `workspace-affinity-migration.md`

- Placement policy and resolved worker are distinct; placement decisions must
  use the same lease, mount, and capability eligibility rules as scheduling.
- Coordinator-owned migrations are durable operations with explicit retry and
  recovery identities. A chat read must not implicitly change workspace
  affinity merely to find spare CPU.

## `authoritative-snapshot-stream-handoffs.md`

- Snapshot-plus-stream consumers must not have a loss window; lag invalidates
  stream authority and requires a resnapshot.
- Existing UI state should survive transport replacement and be replaced by a
  complete new snapshot after reconnect.

The current normalized-log WebSocket contract should therefore remain intact.
Optimization should happen behind it and must never serve a partial cache as a
complete transcript.

## `request-independent-workspace-creation.md`

- Slow work should acquire a durable or server-owned operation identity before
  it leaves the request lifetime.
- A narrow, domain-specific single-consumer operation is preferable to a
  general job platform when one stable identity already exists.
- Restart evidence must prove completion rather than infer it from partial
  artifacts.

For chat materialization, the execution ID is the natural single-flight key.
Only an atomically completed sidecar proves reusable completion; an in-memory
in-flight marker is coordination, not durable truth.

## Consequences for this task

1. Preserve the existing vendor normalizers, patch semantics, WebSocket route,
   bounded input, cancellation discipline, and atomic completed sidecar.
2. Add execution-keyed single-flight coordination around cache misses, with a
   cache recheck after leadership is acquired to close the race.
3. Ensure waiters reuse the completed sidecar/result and failures remove the
   in-flight state so a subsequent request can retry.
4. Keep cache hits outside all expensive-work queues.
5. Do not implicitly migrate affinity or add read-only worker dispatch in this
   increment; first measure the residual cost after deduplication.
6. Add structured observability for cache hits, leader starts, joined waits,
   completion, cancellation/failure, truncation, and durations.
7. Treat the screenshot's coordinator saturation as evidence of the symptom,
   not proof that memory limits or worker placement are the root cause.
