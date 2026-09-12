# Research: Clustered Vibe Kanban

## Existing boundaries

- `services::ContainerService` is the reusable lifecycle seam, but
  `LocalContainerService` still owns child handles, pgids, `MsgStore`s,
  recovery, Git snapshots, setup/cleanup, dev servers, and helpers in one large
  implementation.
- `server/routes/terminal.rs` creates a PTY directly through `Deployment::pty`;
  path visibility alone therefore cannot make terminals remote.
- Preview forwarding connects to coordinator loopback and must gain an
  affinity-aware target rather than only a port.
- Workspace cleanup has separate expiry and orphan sweeps. Reconciliation must
  precede both; worker uncertainty is a retention condition.
- `execution_processes.pgid` is explicitly coordinator-local today. It cannot
  identify a remote process and stays populated only for local compatibility.

## Decisions

### Direct LAN first, protocol independent

Use ordinary authenticated HTTP for registration/control and WebSocket streams
for events/PTY. Existing relay code remains a future carrier. This avoids
binding durable jobs to SSH and keeps retry/cursor semantics above transport.

### Worker journals, coordinator authority

The worker must survive coordinator disconnects and replay output, but it must
not gain database authority. A bounded per-job journal plus terminal metadata
provides supervision evidence; SQLite remains the sole product-state database.

### Idempotency key is the execution UUID plus digest

The coordinator already creates a unique execution record. Reusing that UUID
with the same immutable request returns the existing job. Reusing it with a
different request is a security/state conflict rather than a retry.

### Leases indicate uncertainty, not process death

Heartbeats and leases drive schedulability and UI status. Their expiry cannot
prove a process stopped, so it marks affected work indeterminate and blocks
cleanup until reconciliation.

### Mount health requires identity plus a challenge

Directory existence/writability can be satisfied by a local fallback after NFS
loss. Validate mount table/filesystem identity, expected export identity,
ownership, and a changing coordinator-created probe visible from the worker.

### Git administration stays at one coordinator

NFS locks alone do not fence a stale owner. Since coordinator HA is out of
scope, SQLite-issued repository lock generations plus the coordinator
singleton provide fencing. Workers are not given worktree administration
operations.

### No new dependency initially

Tokio, Axum, Reqwest, Serde, Rustls, existing signing/auth crates, and filesystem
primitives cover the first implementation. A durable embedded worker database
is intentionally avoided; atomic journal files are enough for one daemon and
keep SQLite away from multi-host access.

## Alternatives rejected

- **SSH-supervised agents:** process lifetime and replay would depend on a
  session and reconnect semantics would be weak.
- **SQLite on NFS:** conflicts with the single-writer coordinator contract and
  introduces unsafe multi-host assumptions.
- **One Vibe instance per node:** fragments UI/state and loses sticky,
  coordinator-authoritative lifecycle management.
- **Let workers create worktrees:** races shared `.git/worktrees` metadata.
- **Treat lease expiry as killed:** reports an unverified terminal state and can
  race a still-running writer with cleanup.
- **Migrate workspaces after loss:** explicitly out of scope and unsafe for
  node-local process/tool state.
