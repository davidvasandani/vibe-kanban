# Implementation Plan: Clustered Vibe Kanban

1. Establish the project constitution and feature artifacts under
   `specs/vk/957e-clustered-vibe-k`, carrying forward `SPEC.md` and
   `PRIOR_KNOWLEDGE.md`; resolve protocol, rollout, compatibility, and failure
   semantics before coding.
2. Inventory every process and filesystem boundary in `services`,
   `local-deployment`, `server`, `workspace-manager`, `worktree-manager`,
   executors, terminal, preview, editor/relay, and the Vibe Kanban Nix module.
3. Introduce a transport-neutral cluster protocol crate containing versioned
   worker registration/heartbeat, mount health, dispatch, event, cursor,
   cancellation, approval/input, job inventory, terminal, and preview messages.
   Add serialization and compatibility tests.
4. Add SQLite migrations and DB models for worker nodes, workspace placement,
   and execution ownership/leases. Preserve null/local defaults for existing
   installations, add referential/index constraints, and test migrations and
   state transitions.
5. Implement coordinator worker authentication, registry, heartbeat expiry,
   draining, mount-probe challenges, capability filtering, deterministic
   scheduling, manual selection validation, and admin APIs.
6. Add a worker binary that validates shared-root identity and coordinator
   probes; registers/heartbeats; validates assigned paths; idempotently
   supervises process groups; records ordered bounded events; accepts
   acknowledged cancellation/input; and reports active jobs after reconnect.
7. Refactor local execution behind a dispatcher/supervisor boundary. Keep the
   existing local implementation as the clustering-disabled backend and add a
   remote backend that maps worker events into the existing `MsgStore`,
   persistence, and websocket behavior.
8. Change workspace creation to reserve sticky placement before provisioning,
   construct canonical paths beneath the shared root, and mark the workspace
   runnable only after coordinator-owned worktree and attachment/config setup
   succeeds.
9. Serialize all coordinator Git worktree administration by repository with
   stale-owner/fencing semantics. Reject worker-dispatched administration
   commands and remove unsafe repo-wide concurrency from cleanup paths.
10. Implement execution dispatch/retry, event sequencing/acknowledgement,
    replay-gap handling, truthful cancellation states, worker-lost state, and
    startup/reconnect reconciliation. Ensure reconciliation precedes destructive
    cleanup.
11. Route follow-ups, setup/cleanup/review, approvals/questions, dev servers,
    background helpers, terminals, previews, and editor navigation through the
    persisted worker affinity; centralize scoped environment/secret delivery.
12. Add frontend worker health/admin visibility, placement state, manual worker
    selection, workspace affinity display, and interrupted/indeterminate
    execution states. Regenerate Rust-derived TypeScript types.
13. Extend `modules/vibe-kanban-rebuild.nix` with explicit coordinator/worker
    roles, identical NFS mount options and service ordering, local SQLite
    persistence, consistent UID/GID, secret files, LAN-only worker ports,
    firewall rules, and health/capacity observability.
14. Verify in layers: pure scheduler/path/state tests; DB migrations; protocol
    and in-process disconnect/reconnect integration tests; process-group kill
    and worktree-lock concurrency tests; frontend tests; type generation;
    formatting/lint/checks; Nix evaluation; then a two-disposable-node Slice 2
    validation where available.
15. Run independent Codex review over both Vibe Kanban and its scoped homelab
    module diff, fix confirmed findings, and repeat checks/review until no
    significant findings remain.
16. Distill reusable coordinator/worker, shared-mount, replay, reconciliation,
    and shared-Git safety knowledge into tagged project knowledge-base pages,
    refresh the index, and commit the knowledge-base update.
