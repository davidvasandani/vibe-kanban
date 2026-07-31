# Implementation Plan: Clustered Vibe Kanban

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

Vibe Kanban is a Rust/Tokio/Axum application with SQLite through SQLx and a
React/TypeScript shared frontend. Process ownership currently resides in
`crates/local-deployment/src/container.rs`; the generic surface is
`ContainerService` in `crates/services/src/services/container.rs`. Workspace
paths are stored in `workspaces.container_ref`, terminals bypass the container
launcher through `crates/server/src/routes/terminal.rs`, and preview forwarding
currently assumes coordinator-local loopback.

The implementation adds a Rust workspace crate for versioned transport-neutral
protocol types and a worker binary. Existing `tokio`, `axum`, `reqwest`,
`serde`, `rustls`, `trusted-key-auth`, relay, and WebSocket building blocks are
reused. SQLite remains coordinator-local. The only scoped deployment file is
`../homelab/modules/vibe-kanban-rebuild.nix`.

## Architecture & Approach

### Protocol and ownership boundary

- Add `crates/cluster-protocol` for registration, health, dispatch, ordered
  events, acknowledgements, cancellation, approvals, job inventory, terminal,
  and preview contracts.
- Add `crates/worker` with a `vibe-kanban-worker` binary. It has no `db`
  dependency and accepts only canonical workspace paths below its configured
  shared root.
- Reuse trusted-key signing primitives for direct LAN requests and TLS where
  configured. Include protocol version, request timestamp/nonce, coordinator,
  worker, workspace, and execution IDs in the signed authority.

### Coordinator services

- Add cluster configuration, registry, scheduler, transport client, event
  ingestor, and reconciler under `crates/services/src/services/cluster/`.
- Extend `Deployment`/`LocalDeployment` with those services while retaining the
  current local process path when clustering is disabled.
- Refactor `LocalContainerService` launch/stop boundaries incrementally so local
  and remote execution share execution-record, `MsgStore`, normalization, and
  post-execution Git handling without pretending a remote pgid is local.

### Persistence and provisioning

- Apply `data-model.md` through a forward-only SQLite migration and explicit DB
  transition methods.
- Extend workspace creation inputs with optional placement constraints. Reserve
  placement first, provision the shared path and worktrees on the coordinator,
  then transition to `ready`.
- Add repository-scoped async locks plus a monotonically issued fencing token
  stored in SQLite for every worktree administration operation.
- Run reconciliation before startup expiry/orphan cleanup; both sweeps consult
  worker ownership and retain on uncertainty.

### Streaming, cancellation, and recovery

- Worker journals each execution's bounded event sequence and terminal record
  under shared `execution-logs` with atomic metadata writes.
- Coordinator acknowledges only after an event has entered existing
  persistence/`MsgStore` paths. Reconnect asks for `last_event_sequence + 1`;
  unavailable history marks the transcript incomplete.
- Worker keeps an idempotency record keyed by execution UUID and request digest.
  Same digest returns the existing job; a conflicting reuse is rejected.
- Cancellation is a state machine with graceful, terminate-group, kill-group,
  and confirmed terminal phases. Coordinator timeout produces indeterminate
  state.
- Startup reconciliation compares worker inventories with SQLite and preserves
  conflicting evidence for audit.

### Remaining workspace interactions

- Route all managed process reasons through the dispatcher.
- Move PTY ownership behind a local/remote terminal service; proxy its
  bidirectional frames through the coordinator.
- Extend preview target resolution from a port to `(worker_node_id, job_id,
  port, generation)` so port reuse cannot retarget an old preview.
- Resolve editor/relay destinations from workspace affinity.
- Route approval/question responses through the same correlated cluster client.

### Frontend and deployment

- Generate worker/placement/indeterminate types from Rust.
- Add worker admin and manual placement surfaces in `packages/web-core`, with
  shared local/remote blast radius checked.
- Extend the governing Nix module with explicit cluster role, mount, credential,
  listener, firewall, and service options; keep current defaults disabled.
- Publish `vibe-kanban-worker` in the existing atomic release artifact set.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/worker-protocol.md` and
`./contracts/coordinator-api.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: acceptance criteria map to unit, migration, integration,
  process-group, frontend, and Nix checks.
- Principle III: clustering is disabled by default and delivered behind
  incremental role/configuration boundaries.
- Principles VI/XII: existing `ContainerService`, `MsgStore`, process lifecycle,
  relay host identity, and async ownership patterns are extended.
- Principle XV: cleanup retains on uncertainty and Git administration remains
  coordinator-owned and visible.
- Principle XVIII: affinity, idempotency, cursors, worker evidence, and
  single-owner worktree administration are first-class contracts.
- Generated TypeScript files are regenerated, never hand-edited.

No constitution deviation is required.

## Risks & Dependencies

- The complete feature is production-XL; task layers must remain independently
  testable and rollout-gated.
- NFS availability, snapshots, UID/GID consistency, and mount identity are
  operational prerequisites outside application control.
- Process re-adoption differs by platform and executor; ordinary agents fail
  truthful-interrupted when durable evidence is absent.
- Current SQLx compile-time queries make wide model changes expensive; migrate
  in narrow compilable steps and refresh offline metadata/types.
- Preview and terminal proxying expand the authenticated streaming surface and
  require bounded buffers/backpressure.
- No new top-level third-party dependency is planned. If implementation proves
  an existing auth/locking primitive insufficient, record and review the
  dependency before adding it.
