# Feature Specification: Resource-Aware Chat Loading

**Feature dir**: `specs/vk/6df4-loading-chat-pin/`
**Task id**: `vk/6df4-loading-chat-pin`
**Status**: Clarified

> The checked-in `/speckit.specify` prompt names the already populated
> `specs/vk/a5f8-concat-repeating/spec.md`. This task uses its own branch-derived
> path to preserve that unrelated completed specification.

## Summary

Opening a workspace with a long completed conversation must not pin the Vibe
Kanban coordinator or make the chat appear indefinitely stuck while equivalent
work is repeated. Completed history should be reconstructed once, safely reused
by concurrent and later readers, bounded under large inputs, and observable to
operators. The feature should improve utilization through avoided work and
controlled concurrency without silently moving a workspace or execution to a
different server.

## User Stories

- As a workspace user, I want an existing conversation to open promptly so I
  can review or continue it without waiting behind repeated reconstruction.
- As a user refreshing or opening the same workspace from another browser, I
  want all readers to receive the same complete transcript without multiplying
  server load.
- As an operator, I want chat reconstruction to have bounded concurrency and
  clear diagnostics so one history load cannot obscure the cause of node
  saturation.
- As an operator of a Vibe Kanban cluster, I want spare-node capacity considered
  only through explicit safe placement rules, not through an invisible affinity
  change caused by reading a chat.

## Functional Requirements

- FR-1: A valid completed conversation view must be reused by later readers
  without repeating historical reconstruction.
- FR-2: Concurrent readers requesting the same uncached completed execution
  must share one reconstruction operation.
- FR-3: Every reader that joins a successful reconstruction must receive the
  same complete ordered conversation as the operation owner.
- FR-4: A valid reusable history must bypass the capacity queue for new
  reconstruction work.
- FR-5: Historical reconstruction concurrency, accepted input, produced output,
  and retained coordination state must have explicit bounds.
- FR-6: If older history is omitted to satisfy a bound, the conversation must
  visibly identify that it is partial rather than silently appearing complete.
- FR-7: A failed or canceled reconstruction must not publish partial output as
  complete, must release its coordination state, and must allow a later reader
  to retry.
- FR-8: Disconnecting one reader must not strand another reader. If the
  operation owner disconnects before atomic completion, a waiting reader must
  be able to take ownership and retry; reconstruction with no remaining reader
  must not continue consuming resources indefinitely.
- FR-9: Running executions must continue to stream live state and must not be
  cached as a completed conversation.
- FR-10: Existing conversation content, ordering, patch behavior, and client
  compatibility must be preserved.
- FR-11: Operational diagnostics must distinguish reusable-history hits, new
  reconstruction ownership, joined readers, capacity waiting, completion,
  failure/cancellation, and truncation, and must identify the execution without
  exposing conversation content.
- FR-12: Loading conversation history must not implicitly change workspace
  affinity or treat node metrics as scheduling authority.
- FR-13: Any future cross-node reconstruction must use an explicit authenticated
  Vibe Kanban contract with verified access to the authoritative source; this
  feature must not repurpose agent execution dispatch without that contract.
- FR-14: The change must remain confined to the Vibe Kanban service source and,
  only if necessary, its `modules/vibe-kanban-rebuild.nix` deployment boundary.

## Out of Scope

- Changes to any non-Vibe-Kanban service.
- General-purpose distributed compute or job scheduling.
- Implicit affinity migration based on current CPU or memory readings.
- Replacing vendor-specific log normalizers with a new transcript parser.
- Lossless within-process pagination of legacy raw logs unless the clarified
  solution establishes it as necessary for the acceptance criteria.
- Increasing host CPU or memory limits as the primary remedy.

## Acceptance Criteria

- [ ] Two simultaneous readers of one completed uncached execution cause one
  reconstruction and receive identical complete results.
- [ ] A third reader after completion receives the reusable result without
  entering the reconstruction capacity queue.
- [ ] A leader failure or cancellation leaves no readable partial result and a
  later request successfully retries.
- [ ] Dropping one of multiple readers leaves the remaining reader able to
  complete through cache replay or retry; dropping the final reader stops
  orphaned expensive work within a bounded interval.
- [ ] Running-process history remains live and is never frozen into a completed
  cache.
- [ ] Oversized legacy history remains bounded and explicitly reports omitted
  content.
- [ ] Logs or metrics show cache-hit, leader, joined-waiter, queued, completed,
  canceled/failed, and truncated outcomes by execution ID.
- [ ] Targeted tests, affected workspace checks, formatting, and lint pass; Nix
  evaluation passes if the deployment module changes.
- [ ] Independent Codex review reports no significant unresolved findings.

## Clarified Decisions

- Reconstruction is canceled when its final reader disconnects; completion is
  never kept alive solely as speculative cache warming.
- Single-flight coordination is process-local for the deployed single
  coordinator. The atomic sidecar remains durable truth and is rechecked after
  leadership acquisition.
- Cross-node reconstruction is deferred unless post-change cold-cache p95
  remains above 2 seconds and consumes more than one full core for that period,
  or demonstrably delays live execution traffic. See `clarifications.md`.
