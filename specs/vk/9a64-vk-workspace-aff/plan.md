# Implementation Plan: Workspace Server Affinity and Migration

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Backend: Rust, Axum, SQLite/sqlx in `crates/server`, `crates/services`, `crates/local-deployment`, and `crates/db`.
- Frontend: React/TypeScript, TanStack Query, shared `packages/web-core`, and presentational `packages/ui`.
- Contracts: Rust `serde` + `ts-rs`; generated local types in `shared/types.ts`.
- Existing cluster model: workspace placement fields live on `workspaces`; worker eligibility is centralized in `WorkerScheduler`; coordinator owns execution rows and worker dispatch.
- Constraints: do not move a live process; no per-row affinity requests; no explicit coordinator worker sentinel; block persistent dev/helper processes; preserve session/executor identity; no new dependency.

## Architecture & Approach

### Backend domain service

Add `crates/services/src/services/workspace_affinity.rs` as the coordinator-owned use case. It accepts a workspace, requested target, and `restart_running` flag and returns a typed outcome. The service is responsible for validation and sequencing; the route stays thin.

Use a keyed, coordinator-process workspace lock owned by the service container (the same lifecycle boundary as other process state) so two mutations for one workspace serialize without holding a SQLite transaction across process/network awaits. Under that lock:

1. Read placement and running processes again.
2. Reject any running `DevServer` or `BackgroundHelper`.
3. Resolve the latest/running coding-agent execution and its persisted `ExecutorConfig` from the execution action.
4. Select the target with the existing `WorkerScheduler::select`; `requested_worker_node_id = None` invokes automatic selection.
5. If a coding agent is running and `restart_running` is false, return a typed confirmation-required outcome without mutation.
6. If confirmed, stop that exact execution using `ContainerService::stop_execution` and verify its durable status is no longer `Running`/`Indeterminate` before placement changes.
7. Update placement atomically with a compare-and-set against the previously read placement. The DB primitive updates `worker_node_id`, `requested_worker_node_id`, placement state/reason/timestamps, and fails on a stale concurrent row.
8. Re-provision/verify placement through the existing clustered workspace readiness path as needed before dispatch.
9. Construct one `CodingAgentFollowUpRequest` in the same session with the executor config decoded from the stopped action and a source-owned `WORKSPACE_AFFINITY_MIGRATION_PROMPT`.
10. Start it through the same container execution path as `sessions::follow_up` after extracting a shared follow-up builder/service, so session resume, working directory, cleanup actions, and turn metadata do not drift.

The route `PATCH /api/workspaces/{id}/affinity` in `crates/server/src/routes/workspaces/affinity.rs` maps domain errors to actionable 400/409 responses and returns `WorkspaceAffinityUpdateResponse` for complete and partial results. A restart failure is returned as structured response data rather than rolling placement back.

### Placement summaries

Extend `WorkspaceSummary` in `crates/server/src/routes/workspaces/workspace_summary.rs` with a `WorkspaceAffinitySummary` assembled from the already-loaded workspace placement columns and one bulk `WorkerNode::fetch_all` hostname map. This keeps the existing two archived-status summary requests and adds no per-row calls.

The summary exposes stable facts, not frontend-derived guesses: placement state, assigned worker ID/hostname, requested worker ID/hostname, and a display kind (`local`, `automatic`, `worker`, `unassigned`). `useWorkspaces.ts` maps that into `SidebarWorkspace.serverAffinity`; `packages/ui` only renders the supplied localized label.

### Frontend affinity container

Add `ServerAffinitySectionContainer.tsx` in the workspace page. It fetches detailed placement and the shared worker inventory with host-scoped query keys, derives eligible options with a shared `workerPlacementOptions` helper also used by `CreateChatBoxContainer`, and calls `workspacesApi.updateAffinity`.

The selector keeps a provisional target while confirmation is open, satisfying constitution principle X. The active workspace's canonical `isRunning` is used as an early UX signal; the backend remains authoritative and can still return confirmation-required if state changed.

On success, update/invalidate:

- `['workspacePlacement', workspaceId]`,
- `['workerNodes']`,
- active and archived `workspaceSummaryKeys`,
- execution-process/session queries when a continuation starts.

Add `PERSIST_KEYS.serverAffinitySection` and insert the accordion immediately before Server Metrics in `RightSidebar.tsx`. In non-cluster/local placement, render the current local affinity read-only.

### Left drawer

Extend `WorkspacesSidebarWorkspace` and `WorkspaceSummaryProps` with `serverAffinityLabel`. Render it in the metadata row with truncation and a small server icon, while preserving status icons and diff counts. Both active and archived arrays already share this component.

## Data Model

See `./data-model.md`. No schema migration is expected; the existing workspace placement columns remain authoritative.

## Contracts

See `./contracts/workspace-affinity.md`.

## Research Notes

See `./research.md`. No new dependency is proposed.

## Constitution Check

- I/II/III/VI: thin route, shared placement/scheduler/follow-up paths, explicit acceptance tests, no parallel implementation of lifecycle behavior.
- IV: data/control in `web-core`; row presentation in `packages/ui`.
- X: confirmation dialog owns provisional target; cancel cannot mutate outer state.
- XII: one authoritative coordinator service claims and sequences the asynchronous handoff.
- XIV: generated types and repository verification commands remain mandatory.
- XV: stop must be evidenced before placement; no Git or workspace files are deleted.
- XVIII/XX/XXII: placement stays persisted and affinity-bound; live processes are not moved; migration is one serialized transition with at-most-once continuation and truthful partial outcomes.
- XXI: reuse `WorkerScheduler`, executor-action decoding, and follow-up construction rather than re-deriving their rules; surface entity-specific errors.

No constitution deviation is required.

## Risks & Dependencies

- `try_stop` currently suppresses errors, so migration must use the exact-process stop path and verify durable terminal evidence.
- Execution creation intentionally avoids a SQLite transaction because update hooks need committed visibility; at-most-once restart therefore needs a durable/idempotent claim or deterministic execution identity, not a long transaction.
- Placement readiness/provisioning is currently creation-oriented; extracting a safe reassignment path may reveal assumptions that placement only transitions once.
- Workspace summary requests are host-scoped; cache invalidation must include the current host scope.
- Offline current workers remain displayable but are not valid new targets.
- Full test/build commands are substantial; use focused tests during implementation, then mandatory repository checks.
