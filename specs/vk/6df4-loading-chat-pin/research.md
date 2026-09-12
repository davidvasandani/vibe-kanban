# Research: Resource-Aware Chat Loading

## Current expensive path

For a process without an in-memory `MsgStore`,
`ContainerService::stream_normalized_logs` loads the execution row and checks a
durable normalized-log sidecar. A hit replays one add patch per settled entry.
A miss acquires a process-wide semaphore, loads and caps raw messages, recreates
the workspace if needed, starts the executor's existing normalizer tasks,
deduplicates patches, and materializes final entries only after the consumer
reaches stream completion.

This already bounds memory/concurrency, cancels tasks when the stream is
dropped, and makes later reads cheap. It does not coordinate two misses between
the first lookup and first atomic sidecar write. Multiple browsers or reconnects
can therefore queue duplicate reconstruction for the same execution; because
the cache is not rechecked after the semaphore wait, each queued duplicate pays
the full cost even after the first completed.

## Decision: lock by execution before capacity

Use a weakly retained async mutex per execution ID. The sequence is:

1. optimistic cache read;
2. resolve and acquire the execution cell;
3. cache recheck;
4. acquire global historical capacity;
5. reconstruct, stream, and atomically materialize while retaining ownership.

This is single-flight without inventing a second output bus. Joined readers
wait, then replay the durable result using the already tested cache contract.
If the leader disappears before completion, its lifetime aborts tasks and the
next waiter becomes leader. The durable sidecar—not an in-memory success flag—
decides whether work completed.

Alternatives rejected:

- **Global semaphore plus cache recheck only:** avoids queued duplicate work but
  serializes unrelated executions and cannot distinguish same-key joiners in
  diagnostics.
- **Broadcast normalized patches to every reader:** adds lag, backpressure,
  replay, late-subscriber, and partial-completion semantics duplicating the
  durable cache contract.
- **Shared future containing all patches:** retains the entire transcript in
  memory and complicates cancellation; materialized entries already provide the
  reusable result.
- **Database/file lease:** unnecessary for the deployed single coordinator and
  requires crash expiry/recovery semantics.
- **Worker dispatch:** existing worker jobs are sticky, authenticated live-agent
  operations. A read-only reconstruction job needs a separate protocol and
  cannot be inferred from idle metrics.

## Decision: keep last-reader cancellation

The present stream owns abort handles and the capacity permit; dropping it
cancels normalization. Keep that rule. A waiting reader does not need the
leader's partial stream: if the leader disconnects, the waiter safely retries
as leader. This protects live work from speculative warming and preserves the
atomic-completion boundary.

## Decision: observe rather than raise limits

The screenshot demonstrates high coordinator CPU and low memory, but it does
not identify the exact process or prove that more concurrent normalization
would improve latency. Add safe structured events around wait and work
durations. Existing node/process metrics provide the resource view. The first
optimization is to remove duplicate computation and stale queued misses; no Nix
CPU/memory limit change is justified.

## Dependencies

No new dependency is required. Use Tokio synchronization, `LazyLock`, `Weak`,
`Instant`, and existing tracing/cache modules.
