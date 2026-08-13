# Technical Specification: Resource-Aware Chat Loading

## Objective

Opening a workspace with a long agent conversation can drive the Vibe Kanban
coordinator to approximately 100% CPU for an extended period while other
cluster workers remain lightly loaded and memory pressure remains low. The UI
stays on the conversation loading state during that work. The attached
production observation shows the coordinator at 98.4% CPU with a load near 37,
while schedulable workers are substantially less busy.

Historical conversations are reconstructed from persisted execution logs.
That reconstruction is CPU-intensive and currently occurs on the server that
serves the chat request. This task must make that path resource-aware and avoid
repeating equivalent work while preserving conversation correctness.

## Goals

- Make opening an existing workspace responsive even when its history is long.
- Prevent one chat load, refresh, or group of duplicate readers from creating
  unbounded normalization work on the coordinator.
- Use the resources already available to the Vibe Kanban cluster when doing so
  is compatible with workspace ownership and the existing deployment model.
- Reuse completed normalization work so subsequent readers have near-cache-hit
  cost.
- Keep memory usage bounded and make overload behavior observable.

## Non-goals

- Changing, deploying, or tuning any service other than Vibe Kanban.
- Building a general distributed job platform unrelated to conversation
  loading.
- Changing the semantic content or ordering of agent conversations.
- Moving live agent execution away from its selected workspace worker.

## Functional requirements

1. A request for a completed execution's normalized history MUST reuse a valid
   materialized result when one exists.
2. Concurrent cache misses for the same execution MUST share one normalization
   computation rather than independently parsing and normalizing the same log.
3. Historical normalization MUST have an explicit concurrency bound and MUST
   not block unrelated cache hits.
4. Historical input and materialized output MUST remain bounded, with a visible
   indication when older content is intentionally omitted.
5. A disconnected reader MUST not leave orphaned CPU-heavy work indefinitely
   and MUST not publish a partial result as complete.
6. Running executions MUST remain live and MUST not be frozen into a stale
   completed-history cache.
7. The implementation MUST preserve the existing WebSocket/API contract and
   frontend patch semantics unless the later clarified design demonstrates a
   necessary compatible extension.
8. The operational configuration MUST expose enough information to identify
   which Vibe Kanban process performs expensive reconstruction and whether
   work is queued, deduplicated, completed, or served from cache.

## Resource-utilization design constraints

- Prefer eliminating duplicate work and serving durable materialized history
  over merely raising CPU limits.
- Keep workspace-local filesystem access on the node that owns or can safely
  access the workspace and its session logs.
- Any cluster distribution must use existing Vibe Kanban coordinator/worker
  trust and shared-storage boundaries; it must not introduce dependencies on
  another homelab service.
- Concurrency defaults must be conservative and configurable for heterogeneous
  nodes.
- CPU-heavy synchronous work must not monopolize the async request runtime.

## Acceptance criteria

- Automated tests prove that two simultaneous readers for one completed,
  uncached execution trigger one normalization operation and both receive the
  same completed transcript.
- Automated tests prove valid cache hits bypass the normalization concurrency
  queue.
- Automated tests cover cancellation/failure and demonstrate that a later
  request can retry without reading a partial cache.
- Existing normalized-log cache integrity, truncation, and running-process
  behavior remain covered and passing.
- Relevant Rust and frontend checks pass; Nix evaluation is run if deployment
  configuration changes.
- Runtime logs or metrics distinguish cache hit, shared/in-flight wait, cache
  miss/start, completion, failure/cancellation, and truncation.
- No files outside the Vibe Kanban repository and
  `homelab/modules/vibe-kanban-rebuild.nix` (plus directly related Vibe Kanban
  deployment tests/docs) are changed.

## Investigation questions

- Is the observed CPU consumed primarily by duplicate historical normalizers,
  one intrinsically expensive vendor normalizer, frontend replay/render work,
  or a combination?
- Are persisted session logs available from every worker through the existing
  shared mount, or only from the coordinator/affined worker?
- Does the existing execution-worker dispatch contract support read-only
  normalization jobs, or is single-flight materialization on the serving node
  the safer first increment?
- Which node-level concurrency default best protects interactive agent work
  while still using otherwise idle cores?

## Verification

- Targeted unit/integration tests for normalization caching and concurrent
  readers.
- `cargo test` for affected crates and workspace-level checks in proportion to
  the final diff.
- `pnpm run format`, `pnpm run check`, and `pnpm run lint` when affected code
  requires them.
- Nix module evaluation/tests if the Vibe Kanban deployment module changes.
- Independent Codex diff review with all significant confirmed findings fixed.
