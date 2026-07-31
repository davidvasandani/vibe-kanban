# Feature Specification: Clustered Vibe Kanban

**Feature dir**: `specs/vk/957e-clustered-vibe-k/`
**Status**: Draft

## Summary

Vibe Kanban must present one logical workspace inventory while using healthy
`think-cluster` nodes to run workspace processes. A single coordinator remains
authoritative for application state, workspace provisioning, and Git worktree
administration. Each workspace is placed once on an eligible worker and uses a
shared workspace path visible at the same absolute location on all nodes.
Remote execution must preserve the existing live conversation experience while
reporting disconnects and uncertain outcomes truthfully.

## User Stories

- As a user, I want a workspace to use available cluster compute without
  switching Vibe Kanban instances.
- As a user, I want follow-ups, reviews, terminals, and previews to stay on the
  workspace's assigned node so tools and running processes are consistent.
- As a user, I want live output and reliable cancellation for remote agents so
  clustered execution feels like local execution.
- As an administrator, I want to see worker health and drain or manually select
  nodes so maintenance and placement are controlled.
- As an operator, I want missing mounts, worker loss, and restart ambiguity to
  fail safe so uncommitted work is preserved and false success is impossible.
- As a maintainer, I want one coordinator to own shared Git worktree metadata so
  concurrent workers cannot corrupt it.

## Functional Requirements

- **FR-1:** The system MUST retain exactly one coordinator that owns the UI,
  API, authoritative application database, scheduler, workspace records, and Git
  worktree administration.
- **FR-2:** Each worker MUST have a stable identity and report hostname,
  lifecycle state, compatible versions, compute/load information, supported
  executors, active execution count, labels, and shared-volume health.
- **FR-3:** Offline, draining, incompatible, shared-volume-unhealthy, or
  executor-ineligible workers MUST be excluded from automatic placement.
- **FR-4:** A valid manually requested worker MUST take precedence; otherwise
  eligible workers MUST be ranked by configured load and active-execution
  weights with a deterministic tie break.
- **FR-5:** Placement MUST occur and be persisted before workspace
  materialization. The workspace MUST retain the same worker for its lifetime.
- **FR-6:** A workspace MUST NOT become runnable until its shared directory,
  worktrees, attachments, and generated configuration are successfully
  provisioned.
- **FR-7:** All workspace process categories—including setup, coding agents,
  follow-ups, reviews, cleanup, helpers, dev servers, and terminals—MUST execute
  on the persisted worker.
- **FR-8:** The worker MUST reject a request whose worker identity, workspace
  identity, execution identity, or canonical shared path is not authorized by
  the coordinator's assignment.
- **FR-9:** Starting and cancelling MUST be idempotent by coordinator execution
  ID. Repeating a start MUST NOT create a duplicate process.
- **FR-10:** Workers MUST emit ordered, cursor-addressable lifecycle, output,
  structured message, approval, preview, completion, failure, kill, and
  indeterminate events.
- **FR-11:** The coordinator MUST resume event delivery from its last
  acknowledged cursor after reconnect and MUST visibly report an unrecoverable
  replay gap.
- **FR-12:** Worker output MUST feed the same persisted conversation/log and
  live UI channels as coordinator-local execution.
- **FR-13:** Cancellation MUST progress through graceful cancellation and timed
  process-group escalation, remain safe to repeat, and report killed only after
  worker confirmation.
- **FR-14:** Approvals and questions MUST correlate the user response and worker
  acknowledgement to one execution and one approval/request ID, with explicit
  disconnect behavior.
- **FR-15:** Preview, terminal, and remote-editor routing MUST resolve the
  workspace's persisted worker instead of a transient UI host selection.
- **FR-16:** After coordinator reconnect, workers MUST report active jobs and
  the coordinator MUST reconcile them with authoritative execution records.
- **FR-17:** An execution reported running in the database but absent from its
  reachable worker MUST become interrupted or indeterminate, never completed.
- **FR-18:** Unknown worker jobs MUST be quarantined by default. A returning
  worker MUST NOT silently override a newer coordinator terminal decision.
- **FR-19:** Destructive cleanup MUST be deferred whenever the assigned worker
  is active, unreachable, or ownership is otherwise uncertain.
- **FR-20:** Every eligible node MUST see the shared volume at the same absolute
  path and MUST verify expected mount identity, a coordinator probe, required
  writability, and `vibe-kanban` ownership before accepting work.
- **FR-21:** The authoritative application database MUST remain
  coordinator-local and MUST NOT be placed on the shared volume.
- **FR-22:** Only the coordinator MAY add, remove, or prune Git worktrees,
  reclaim workspace directories, or delete workspace branches. These operations
  MUST be serialized per repository with stale-owner/fencing protection.
- **FR-23:** Worker connections and operations MUST be authenticated,
  authorized, replay-resistant, auditable, and support credential rotation.
- **FR-24:** Execution secrets MUST be scoped to one dispatch and MUST NOT be
  persisted in worker logs or shared storage.
- **FR-25:** When clustering is disabled, existing single-node workspace and
  execution behavior MUST remain available.
- **FR-26:** Administrators MUST be able to view worker health and
  unschedulable reasons, drain workers, configure scheduling, and request manual
  placement.

## Out of Scope

- Moving a live workspace between nodes.
- Multiple active coordinators or coordinator high availability.
- Concurrent access to one SQLite database from multiple hosts.
- Automatically continuing an ordinary coding-agent process on a different
  worker after loss.
- Replicating shared storage or changing its backing storage service.
- Scheduling non-Vibe workloads, autoscaling, or preemption.
- Replacing existing relay remote-access behavior.
- Using an SSH session as the durable execution supervisor.
- Updates to any non-Vibe-Kanban service.

## Acceptance Criteria

- [ ] From one UI, a user creates workspaces assigned to either of two eligible
      disposable nodes and sees their files immediately on the coordinator.
- [ ] Automatic placement excludes every specified unhealthy/ineligible state;
      valid manual placement wins and invalid manual placement fails visibly.
- [ ] Every later execution and interaction for a workspace targets its original
      worker.
- [ ] Retrying the same dispatch starts one process, and live ordered logs
      survive a coordinator disconnect/reconnect.
- [ ] A replay window gap appears as incomplete output rather than a complete
      transcript.
- [ ] Cancellation terminates a remote child process group and reports a
      terminal kill only after acknowledgement.
- [ ] Coordinator restart reconnects to active jobs or marks missing jobs
      interrupted/indeterminate without inferring success.
- [ ] Worker loss blocks cleanup and visibly marks affected executions.
- [ ] A missing, masked, read-only, wrong-filesystem, probe-invisible, or
      ownership-invalid shared mount makes a worker unschedulable.
- [ ] Concurrent workspace provisioning/removal cannot concurrently mutate one
      repository's Git worktree metadata.
- [ ] Terminal input/output and previews reach the assigned node.
- [ ] SQLite remains local to the coordinator.
- [ ] Clustering-disabled installations pass existing local execution tests.
- [ ] Only Vibe Kanban source and its governing homelab module are changed.

## Clarified Decisions

- The product specification covers all five slices and the final acceptance
  criteria. Implementation is feature-gated and delivered in dependency order,
  with the two-node Slice 2 experiment as the first rollout gate rather than the
  final scope.
- The homelab module accepts coordinator/worker credential file paths and passes
  them with systemd `LoadCredential`; credential generation and rotation stay
  with the homelab's existing secret owner. No credential is generated into or
  embedded in the Nix store.
- The production default mount path is `/srv/vibe-kanban-shared`, backed by
  `172.16.0.99:/var/nfs/shared/VibeKanban/mnt`, and remains configurable only as
  one shared-root option that must match across coordinator and workers.
- Direct authenticated LAN HTTP/WebSocket is the implemented carrier. Protocol
  boundaries retain addressed-worker semantics so relay carriage can be added
  later without changing execution semantics.

## Open Questions

None.

## Success Metrics

- Two-node Slice 2 validation proves idempotent remote agent start, ordered
  replay after disconnect, authoritative cancellation, and safe coexistence with
  coordinator-managed worktrees.
- No clustered failure case is represented as successful without worker
  evidence.
- Existing single-node users require no migration of their database or
  workspace behavior until clustering is explicitly enabled.
