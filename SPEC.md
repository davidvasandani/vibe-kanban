# VK Soft Restart Resilience — Technical Specification

> Delivery note: the shipped increment uses the already-separate cluster worker
> as the stable owner for coding-agent executions. PTY session journaling and
> standalone/local-server process preservation remain follow-up work.

## Summary

Vibe Kanban deployments currently replace the server process. The browser loses its API/event connections and renders a blank failure state while every locally attached coding-agent and dev-server process is terminated. This feature introduces a stable bootstrap/supervision boundary so the replaceable application server can restart without owning or killing active execution processes. The web client will tolerate the bounded control-plane outage, reconnect, and reconcile with the same live executions.

The implementation is limited to the Vibe Kanban source repository and its deployment definition in `homelab/modules/vibe-kanban-rebuild.nix`.

## Problem

The current runtime combines three lifetimes:

1. the HTTP/WebSocket application server,
2. the execution-process supervisor and its agent/dev-server children, and
3. the deployed release selected by systemd.

Restarting the systemd service therefore tears down all three. Database recovery can label and resume interrupted work, but that is compensation after process death; it does not preserve an agent's in-memory conversation, subprocess tree, pending tool interaction, or streaming output. At the same time, transient API and event-stream failures can leave the browser on a white page rather than an explicit reconnecting state.

## Goals

- Preserve running coding agents, background helpers, and dev servers across an application-server restart.
- Keep every preserved child continuously attached to a long-lived supervisor; do not orphan subprocesses or rely on accidental OS behavior.
- Reattach the restarted application server to the supervisor's authoritative process inventory and live output streams.
- Preserve execution IDs and database-visible `running` state across a successful soft restart.
- Buffer enough process output and terminal status to bridge a bounded application outage without silent loss or duplication.
- Make the browser remain usable during the outage by displaying a reconnecting state and automatically restoring API/event subscriptions.
- Retain a deliberate hard-stop path that terminates managed children for host shutdown, operator request, incompatible upgrade, or supervisor failure.
- Integrate the soft-restart sequence with the existing release activation, health gate, and rollback behavior.

## Non-goals

- Preserving agents across a host reboot, kernel failure, or supervisor restart.
- Migrating active agents between hosts.
- Changing execution semantics for remote/cloud services outside the Vibe Kanban deployment.
- Hot-reloading Rust code in-process or emulating JVM classloader replacement.
- Guaranteeing uninterrupted individual HTTP requests during the application handoff.

## Proposed Architecture

### Stable supervisor/bootstrap

A small, version-compatible supervisor process becomes the systemd-owned long-lived runtime. It owns spawned execution subprocesses, process-group cleanup, output capture, and an IPC endpoint. The application server becomes a replaceable child/client of that supervisor. Agent stdin, stdout/stderr readers, cancellation handles, and exit watchers live on the supervisor side.

The IPC protocol must support:

- capability/version negotiation,
- spawn, write/input, stop, and query operations,
- a full authoritative inventory snapshot,
- ordered output events with per-process sequence numbers,
- replay from a caller-provided sequence/cursor,
- terminal-state delivery and acknowledgement,
- application-instance attach/detach without affecting executions,
- supervisor drain and hard-shutdown operations.

The supervisor persists process metadata and bounded replay buffers required to survive application detachment. SQLite remains the durable business-state store; reconciliation after attachment updates stale rows from supervisor truth without changing the identity of live executions.

### Replaceable application server

On startup, the server negotiates with and attaches to the supervisor before accepting traffic. It reconciles database execution rows with the supervisor inventory, resumes output consumption from stored/in-memory cursors where possible, and exposes readiness only after reconciliation. On soft shutdown it stops accepting new work, drains requests, detaches from the supervisor, and exits without cancelling managed executions.

Execution launch and control paths are routed through a supervisor client abstraction. Existing in-process execution ownership may remain available for tests and explicit standalone/development mode, but production deployment must use the external supervisor.

### Deployment handoff

Release activation uses an explicit soft-restart command rather than restarting the supervisor unit. The supervisor launches or is instructed to launch the new application release, waits for its readiness, and only then retires the old application instance. If the candidate fails its health gate, the supervisor keeps or restores the previous application release while active executions remain untouched.

Protocol incompatibility is detected before handoff. An upgrade that requires replacing the supervisor must fail closed with a clear operator-visible reason unless an explicit hard restart is requested.

### Browser resilience

The SPA treats transport loss as a recoverable control-plane outage. Already-rendered workspace and conversation state remains visible. A non-destructive reconnect banner/overlay communicates the restart, requests retry with bounded exponential backoff and jitter, and event subscriptions resume using their cursor/replay mechanism. Full-page reload is a last resort only for a newly deployed incompatible frontend asset version, and must be coordinated after server readiness.

## Functional Requirements

1. A soft restart while one or more coding agents are running leaves their OS PIDs/process groups alive.
2. Preserved executions retain their execution-process IDs and remain `running`; they are not marked `interrupted` merely because the application detached.
3. Prompts/input and stop requests work after the new application instance attaches.
4. Output generated during the handoff appears after reconnection in order and at most once in the canonical stored log.
5. A child that exits while no application instance is attached has its terminal state delivered and durably reconciled after attachment.
6. New execution launches are rejected or queued with an explicit retryable response while no ready application instance can safely service them.
7. Browser routes do not turn blank solely because health/API/WebSocket/SSE connections fail during a restart.
8. Rollback after a failed candidate application preserves active executions.
9. Explicit hard shutdown terminates managed process groups and records correct terminal/interrupted state according to existing policy.
10. Existing startup recovery remains available for genuinely orphaned rows after a supervisor/host failure.

## Safety and Consistency Invariants

- Exactly one supervisor is authoritative for a local execution at a time.
- Application detachment never implies process cancellation.
- Spawn requests are idempotent across an uncertain IPC response, keyed by execution ID/request token.
- Output ordering is scoped per execution and represented by monotonically increasing sequence numbers.
- Reconciliation prefers observed supervisor state over stale database `running` rows, while never adopting an unknown process without validated metadata.
- Process termination targets validated process groups owned by the supervisor, not broad cgroups or unresolved PIDs.
- Authentication/authorization remains enforced by the application; the local IPC endpoint is filesystem-permission protected and validates peer identity where supported.

## Failure Handling

- **Application crash:** supervisor retains children and replay buffers; systemd/supervisor launches a replacement application.
- **Candidate fails readiness:** abort handoff and continue/restore the prior application.
- **IPC disconnect:** application enters degraded/not-ready state and does not infer child death.
- **Supervisor crash:** existing startup orphan recovery marks unverifiable executions interrupted; the deployment reports that a hard interruption occurred.
- **Replay overflow:** surface a detectable gap, reconcile from durable logs where possible, and mark output incomplete rather than silently continuing.
- **Protocol mismatch:** refuse soft restart and require an explicit compatible deploy or hard restart.

## Observability

Structured logs and metrics must distinguish soft application restarts from supervisor/hard restarts and include application generation, supervisor generation, attach duration, preserved execution count, replayed event count, replay gaps, failed reconciliations, candidate readiness failures, and rollback outcomes. The health/readiness endpoint must expose whether supervisor attachment and initial reconciliation are complete.

## Verification and Acceptance Criteria

- Unit tests cover IPC negotiation, idempotent spawn, ordered replay, terminal events during detachment, reconciliation, buffer gaps, and hard shutdown.
- Integration tests launch a long-running fake agent, replace the application instance, verify the same child PID and execution ID survive, and verify pre/during/post-handoff output exactly once.
- Deployment tests cover successful handoff, candidate health failure/rollback, protocol mismatch, and explicit hard restart.
- Frontend tests simulate API/event transport loss and recovery, verifying cached content and reconnect UI remain rendered.
- A manual staging drill runs at least one real coding-agent turn and dev server through a deployment and confirms continued interaction afterward.
- Normal repository formatting, type checks, targeted Rust/frontend tests, and Nix evaluation for `vibe-kanban-rebuild.nix` pass.

## Rollout

Ship behind an opt-in deployment setting. Initially run supervisor mode on the Vibe Kanban host with additional reconciliation logging, retain the existing hard-restart action, and document downgrade behavior. Enable soft restart by default only after restart/rollback drills demonstrate preserved processes and bounded browser recovery.

## Open Questions for Discovery and Clarification

- Which existing execution ownership abstractions can move behind IPC without duplicating executor-specific logic?
- Does the current log persistence path already provide a durable replay cursor, or is a supervisor-side journal required?
- Can systemd socket activation or a dedicated supervisor/application unit split provide the safest release handoff within the current Nix module?
- Which frontend transport and query layers currently clear rendered state on disconnect?
- What compatibility window is required between supervisor and application releases?
