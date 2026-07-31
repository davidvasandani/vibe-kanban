# Technical Spec: Clustered Vibe Kanban with Shared Workspace Storage

## Summary

Add a coordinator/worker execution model to the self-hosted Vibe Kanban
deployment. One coordinator remains authoritative for the UI, API, SQLite,
workspace records, scheduling, and Git worktree administration. Eligible
`think-cluster` nodes run an authenticated worker daemon that supervises
workspace processes on a shared NFS volume mounted at the same absolute path on
every node.

Workspace placement occurs before provisioning and is sticky for the lifetime of
the workspace. The worker protocol provides idempotent dispatch, ordered and
replayable execution events, cancellation with acknowledgement, health and mount
validation, and restart reconciliation. Existing coordinator-local execution
remains available when clustering is disabled.

## Scope

- Vibe Kanban source in this repository.
- Deployment changes only in the Vibe Kanban service's governing homelab module,
  `modules/vibe-kanban-rebuild.nix`, and its directly related Vibe Kanban
  configuration.
- Coordinator-owned worker registry, health tracking, draining, scheduling, and
  sticky workspace affinity.
- A new lightweight worker binary/service for process supervision.
- Shared workspace roots under one configurable identical absolute path.
- Remote coding-agent execution with ordered output, completion, cancellation,
  and reconnect/reconciliation semantics.
- Coordinator-only, repository-scoped serialization of worktree administration.
- Host-aware follow-ups, setup/cleanup/review processes, terminal sessions,
  previews/dev servers, editor routing, and approvals.
- UI visibility and manual placement controls for workers and affected
  workspaces/executions.
- Backwards-compatible single-node operation when no remote workers are enabled.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Multiple active coordinators or shared SQLite.
- Live migration of a workspace or running process between workers.
- Automatic continuation of an ordinary coding-agent process on another worker.
- Storage replication, Kubernetes-style orchestration, autoscaling, or
  preemption.
- Replacing the existing relay remote-access product.
- Treating SSH sessions as durable process supervision.

## Architecture

### Coordinator

The existing Vibe Kanban server remains the only owner of SQLite and application
state. It:

1. authenticates workers and records their advertised capabilities and health;
2. selects a worker before materializing a workspace;
3. persists placement and shared paths transactionally enough that an
   incompletely provisioned workspace is not runnable;
4. exclusively creates/removes/prunes Git worktrees under repository-scoped
   fenced locks;
5. creates execution records and dispatches immutable, idempotent execution
   requests;
6. consumes ordered worker events into the existing `MsgStore`, persistence,
   normalization, and WebSocket paths;
7. routes input, approvals, cancellation, terminals, previews, and editor
   actions to the workspace's persisted worker; and
8. reconciles worker-reported jobs after either side reconnects.

### Shared volume

The coordinator and workers use a configured root such as
`/srv/vibe-kanban-shared` backed by
`172.16.0.99:/var/nfs/shared/VibeKanban`. It contains repositories,
workspaces, and optional durable execution logs, but never the coordinator's
SQLite database.

Every schedulable node validates that the configured path is the expected NFS
mount rather than a local fallback, can observe a coordinator-issued probe, has
the expected filesystem identity, and has correct `vibe-kanban` UID/GID access.

### Worker

The worker is a separate Rust binary with no SQLite access and no authority to
administer Git worktrees. It validates all requested paths against the configured
shared root and the workspace assignment carried by an authenticated dispatch.
It supervises process groups, stores a bounded replay buffer or durable event
log, accepts correlated input/approval responses, escalates cancellation, and
reports authoritative terminal state.

### Transport

The initial carrier is authenticated LAN HTTP plus WebSocket on the flat cluster
network. Protocol types and transport-facing traits remain independent enough
for the existing relay to carry addressed worker traffic later. Requests include
coordinator identity, worker identity, execution ID, monotonic sequence/cursor,
and anti-replay authentication.

## Data Model

### `worker_nodes`

- stable `id` and `hostname`;
- lifecycle status (`online`, `offline`, `draining`);
- worker and compatible Vibe Kanban versions;
- last heartbeat and lease expiry;
- capabilities/executor profiles and optional labels;
- CPU count/load, available memory, and active execution count;
- shared-mount validation state and diagnostic reason.

### Workspace affinity

Add `worker_node_id`, placement state, placement timestamp, placement reason, and
optional requested-node/constraint data to workspaces. A runnable workspace has
exactly one persisted worker and a canonical path within the shared workspace
root. Placement cannot change while the workspace exists in the first release.

### Execution ownership

Add worker node ID, worker job ID, last acknowledged event sequence, and lease
expiry to execution ownership records. Dispatch identity is the coordinator
execution UUID; a worker must return the existing job for a repeated start with
the same ID and reject mismatched payload reuse.

## Functional Requirements

### Registration, health, and scheduling

1. Workers authenticate, register stable identity/capabilities, and heartbeat.
2. Offline, draining, incompatible, executor-missing, or mount-unhealthy workers
   are never selected automatically.
3. A valid manual request wins; otherwise scheduling ranks eligible workers by
   configurable weighted load and active execution count, with a deterministic
   tie break.
4. Health state and the reason a worker is unschedulable are visible to
   administrators.

### Workspace provisioning

1. Placement is reserved and persisted before directory/worktree creation.
2. The coordinator creates the shared workspace and all worktrees while holding
   repository-scoped administration locks with stale-owner/fencing semantics.
3. Attachments and generated configuration are copied before the workspace is
   marked runnable.
4. Failures produce an explicit provisioning-failed state and preserve data for
   diagnosis; no execution is dispatched.
5. Cleanup is forbidden while the assigned worker reports activity or is
   unreachable and ownership is uncertain.

### Dispatch and events

1. Dispatch carries execution/workspace/session IDs, validated shared path,
   executor action/profile, working directory, environment and scoped secrets,
   reason, timeout, and persistence policy.
2. Start and cancellation are idempotent by execution ID.
3. Worker events are monotonically sequenced and cover accepted, starting,
   stdout, stderr, structured executor messages, approval/question requests,
   preview metadata, completed, failed, killed, and indeterminate/worker-lost.
4. The coordinator acknowledges persisted sequences and resumes from its last
   acknowledged cursor after reconnect.
5. A bounded worker replay window is retained; a cursor gap is surfaced as
   incomplete output rather than silently ignored.
6. Secrets are neither included in persisted dispatch diagnostics nor written to
   shared execution logs.

### Cancellation and interaction

1. Cancellation attempts executor-specific graceful shutdown, then timed
   process-group termination and force kill.
2. Coordinator timeout or disconnect is not represented as a confirmed kill.
3. Approval/question messages correlate approval ID, execution ID, type,
   deadline, response, and worker acknowledgement; disconnect behavior is
   explicitly fail-closed, pause, or timeout per executor capability.
4. Terminal sessions and dev-server processes run on the assigned worker.
5. Preview proxying and editor links resolve the workspace's persisted worker,
   not a currently selected UI host.

### Recovery

1. On coordinator startup/reconnect, each worker reports active and retained
   jobs. Matching records resume from the last acknowledged sequence.
2. A SQLite-running execution absent from its assigned online worker becomes
   interrupted or indeterminate; it is never inferred complete.
3. Unknown worker jobs are quarantined by default and may be terminated only by
   explicit configured policy.
4. After worker restart, durably supervised persistent jobs may be re-adopted;
   ordinary unsupervised coding-agent jobs become interrupted.
5. A worker returning after loss cannot overwrite a newer terminal coordinator
   decision; conflicting evidence is retained for audit.

## Security Requirements

- Mutual worker/coordinator authentication with rotatable credentials.
- Authorization binds every operation to the persisted workspace/worker and
  execution IDs.
- Canonicalized paths must remain beneath the configured shared root; symlink
  escapes and arbitrary paths are rejected.
- Signed/nonce-protected dispatch and cancellation requests prevent replay.
- Secrets are scoped to one execution and redacted from logs.
- Audit logs include coordinator, worker, workspace, execution, dispatch, and
  cancellation identifiers without secret values.
- The shared SSH identity may bootstrap credentials but is not the steady-state
  application credential.

## Deployment Requirements

- Add coordinator and worker options to `modules/vibe-kanban-rebuild.nix`.
- Keep `db.v2.sqlite` on coordinator-local persistent storage.
- Mount the NFS export at one configurable identical absolute path on think2 and
  each enabled worker node, with ordering that prevents service start on a local
  fallback directory.
- Run services as `vibe-kanban` with consistent UID/GID.
- Default network exposure to the cluster LAN and firewall only coordinator to
  worker protocol ports.
- Provide credential files through the existing Vibe Kanban secret mechanism,
  not the Nix store.
- Add capacity/mount health observability and document snapshot/recovery
  prerequisites without configuring unrelated storage services.

## Delivery Strategy

Implementation remains sliceable behind configuration:

1. worker visibility, authentication, mount health, and draining;
2. sticky placement plus one remote coding-agent path with stream/cancel;
3. remaining workspace interactions, approvals, terminals, previews, and editor;
4. replay, leases, reconciliation, and operational safeguards;
5. weighted scheduling, labels, constraints, and manual overrides.

The first deployable validation must run Slice 2 on two disposable nodes. Later
slices may build on the same protocol without weakening the failure semantics
specified here.

## Acceptance Criteria

- One UI creates a workspace on any eligible enabled node and immediately sees
  its shared files from the coordinator.
- Placement is persisted before provisioning and all subsequent workspace
  process types remain on that node.
- Duplicate dispatch cannot start a second process.
- Live logs preserve order, reconnect from an acknowledged cursor, and remain
  available after completion.
- Cancellation kills the remote process group only when worker acknowledgement
  establishes the terminal result.
- Coordinator restart reconnects or truthfully marks executions interrupted or
  indeterminate.
- A missing, masked, read-only, identity-mismatched, or ownership-invalid mount
  makes a worker unschedulable.
- Worker loss is visible and prevents unsafe cleanup.
- Terminal, preview, and editor routing use persisted affinity.
- Concurrent workspaces on different nodes cannot concurrently mutate shared
  Git worktree administration metadata.
- SQLite remains local to the coordinator and no other service is changed.
- Clustering-disabled installations retain current single-node behavior.

## Verification

- Unit tests for eligibility/scoring, sticky placement, path authorization,
  idempotency, event ordering/replay gaps, cancellation transitions, mount
  validation, and reconciliation.
- Database migration/round-trip tests and generated TypeScript type checks.
- Protocol integration tests with an in-process coordinator and worker,
  including disconnect/reconnect and duplicate delivery.
- Process-group cancellation test using a child/grandchild fixture.
- Repository-lock concurrency test.
- Frontend tests for worker state, manual selection, and indeterminate status.
- Nix evaluation/tests for coordinator/worker roles, mount dependencies,
  credentials, users, and firewall rules.
- Formatting, lint, relevant Rust/TypeScript test suites, and an independent
  Codex diff review.

## Risks and Mitigations

- **Scope size:** preserve delivery-slice boundaries and backwards-compatible
  local execution so partial rollout is operable.
- **Split-brain execution:** immutable sticky affinity, idempotent dispatch,
  leases, and worker-side assignment validation.
- **Shared Git corruption:** coordinator-only administration plus fenced
  repository locks.
- **False healthy mount:** verify mount identity and a coordinator probe, not
  merely directory existence/writability.
- **Lost or duplicated output:** monotonic sequences, acknowledgement, replay,
  and explicit cursor-gap state.
- **Incorrect success after disconnect:** terminal states require worker
  evidence; otherwise report interruption or indeterminacy.
- **Unsafe cleanup:** preserve workspace data whenever worker liveness or process
  ownership is uncertain.
