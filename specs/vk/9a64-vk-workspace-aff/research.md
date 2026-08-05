# Research: Workspace Server Affinity and Migration

## Existing seams

- `WorkspacePlacement` already stores assigned and requested worker IDs, placement state, timestamps, constraints, and reason (`crates/db/src/models/workspace.rs`). No new affinity table is needed.
- Initial placement uses `WorkerScheduler::select` and `WorkspacePlacement::reserve` in `crates/server/src/routes/workspaces/create.rs`. The same selector must decide both explicit and automatic reassignment.
- `stop_workspace_execution` delegates to `ContainerService::try_stop`, which is deliberately best-effort and suppresses errors. Migration needs the exact `stop_execution` API plus a durable status reread.
- Session follow-ups in `crates/server/src/routes/sessions/mod.rs` recover the prior agent session, working directory, cleanup action, and executor configuration. This logic should be extracted so migration and user follow-ups share it.
- Workspace summaries already bulk-load workspaces and derived execution state. Placement columns are on those workspace rows; one bulk worker read can resolve every hostname.
- The carousel's `PlacementLabel` proves the display vocabulary but performs per-column placement fetching. The left drawer cannot repeat that pattern at list scale.

## Decisions

### Coordinator-owned service, not client choreography

Chosen because stop, placement, and restart cross several durable boundaries. A browser sequence can be interrupted or retried between calls and create false state or duplicate continuations. The constitution explicitly requires one owner.

### Process-local keyed serialization plus durable restart claim

A keyed lock prevents concurrent requests in one coordinator, but HTTP retry after response loss can arrive after lock release. At-most-once continuation therefore also needs durable evidence. Preferred implementation: add a migration operation/claim row only if no existing execution idempotency field can represent the request; otherwise derive a stable continuation execution ID from a server-issued operation ID returned/accepted by the endpoint. Planning must choose the smallest durable mechanism after inspecting execution creation APIs.

### Immediate scheduling for automatic affinity

Automatic is resolved at mutation time so the drawer immediately reflects a real server. `requested_worker_node_id` stays null, preserving the distinction between scheduler choice and explicit preference.

### Block persistent processes

Dev servers and helpers are not coding-agent turns and have no generic safe recreation contract. Stopping without restarting them would be surprising; leaving them on the old worker would make affinity false. Blocking is the smallest truthful behavior.

### No explicit coordinator option

The cluster coordinator is not a worker-node record and cannot accept worker-protocol dispatch. Non-cluster installations remain local and informational.

## Alternatives rejected

- **Client performs stop/update/follow-up:** not atomic or idempotent across response loss.
- **Mutate only `requested_worker_node_id`:** the current assigned worker and UI remain stale, so the promised affinity change is not observable.
- **Move a live process:** process, logs, cancellation, terminal, and worker-job state are node-owned and cannot be transferred.
- **Restart all persistent processes:** arbitrary helper commands and dev-server context do not share a safe generic restart contract.
- **Per-row placement queries:** creates N+1 traffic and duplicates worker inventory reads.
- **New state-management dependency:** TanStack Query and existing stores cover the use case.

## Dependency decision

No new Rust or npm dependency.
