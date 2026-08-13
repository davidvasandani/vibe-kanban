# Implementation Plan: Resource-Aware Chat Loading

Task: `vk/6df4-loading-chat-pin`

1. Establish the SpecKit constitution and task-scoped specification artifacts,
   then reconcile their requirements with `SPEC.md` and
   `PRIOR_KNOWLEDGE.md`.
2. Instrument and test the completed-execution cache-miss seam in
   `ContainerService::stream_normalized_logs`, confirming where normalization
   tasks run, how permits are held, and what happens when a reader disconnects.
3. Introduce a process-wide, execution-ID-keyed historical-materialization
   coordinator with explicit leader/joiner outcomes. Keep the abstraction
   narrow: it coordinates one durable normalized sidecar, not arbitrary jobs.
4. Recheck the durable cache after becoming the single-flight leader. This
   closes the first-read race between the optimistic cache lookup and ownership
   acquisition, including completions from another request or process sharing
   the same session storage.
5. Refactor the cache-miss normalization path so one leader performs bounded
   reconstruction and publishes only an atomically complete materialization.
   Joined readers await that outcome and replay the same completed entries.
6. Define cancellation ownership deliberately: an individual joined reader may
   disconnect without canceling work needed by other readers; when no consumers
   remain, expensive tasks must be aborted and the in-flight entry cleared.
   Failure and cancellation must remain retryable and must never make a partial
   sidecar readable.
7. Keep valid cache hits ahead of both single-flight coordination and the
   historical-normalization semaphore. Run any newly isolated synchronous
   materialization work on an appropriate blocking/CPU boundary so the Tokio
   request runtime remains responsive.
8. Add structured tracing around cache hit/miss, leader acquisition, joined
   waiters, queue duration, normalization duration, completion, cancellation,
   failure, message count, and truncation. Reuse existing node/process metrics
   rather than adding another monitoring service.
9. Add focused concurrency tests proving one normalization for simultaneous
   readers, equal completed output for all readers, cache-hit bypass, retry
   after leader failure/cancellation, and no partial cache publication. Retain
   existing integrity, truncation, and running-process tests.
10. Benchmark or otherwise measure the targeted reconstruction seam with a
    representative long transcript. If single-flight plus durable reuse leaves
    material coordinator saturation, document a follow-up distributed
    normalization protocol; do not overload the existing agent execution-job
    dispatch contract in this task.
11. Run repository-required setup, formatting, targeted tests, `pnpm run check`,
    and lint. Evaluate Vibe Kanban Nix configuration only if the deployment
    module changes.
12. Run SpecKit analysis before implementation and, after implementation, an
    independent Codex diff review. Address confirmed significant findings and
    repeat verification/review until clean.
13. Update the Vibe Kanban knowledge base with reusable single-flight and
    materialization findings, tag it `vk/6df4-loading-chat-pin`, refresh its
    index, and commit the knowledge-base update.
14. Rebase or otherwise confirm the latest base tip, commit the scoped changes,
    open a pull request, wait for required checks/review, merge it, and report
    the merged result.
