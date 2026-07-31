# Cluster validation evidence

## 2026-07-31 — Slice 2 disposable-node gate

The implementation and NixOS evaluation were validated locally, but a live
two-disposable-node Slice 2 exercise was not run from this workspace.

Read-only deployment discovery established that:

- the homelab configuration does not yet assign `clusterRole = "coordinator"`
  or `clusterRole = "worker"` to a real host; those assignments exist only in
  `tests/vibe-kanban-cluster.nix`;
- `think3` and `think4` are reachable over SSH, but neither has an active
  `vibe-kanban-worker` unit or a filesystem mounted at
  `/srv/vibe-kanban-shared`; and
- the reachable machines are existing homelab nodes, not disposable validation
  targets, so mutating or redeploying them solely for this test would exceed
  the safe validation scope.

The live acceptance exercise therefore remains a deployment-time gate. It must
use two explicitly designated disposable nodes and follow
`docs/self-hosting/clustered-workers.mdx`, covering ordered replay after a
coordinator disconnect, idempotent dispatch, graceful/forced cancellation,
worker-loss reconciliation, and coordinator-owned worktree administration.

