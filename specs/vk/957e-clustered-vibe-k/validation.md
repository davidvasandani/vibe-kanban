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

## 2026-07-31 — Authorised think3/think4 trial preflight

The operator subsequently designated `think3` and `think4` as the disposable
validation nodes. Read-only and reversible preflight checks established that:

- both nodes are reachable, run `x86_64` NixOS, and have passwordless sudo for
  the validation operator;
- neither node has a Vibe Kanban worker binary or active worker service;
- the existing `vibe-kanban` accounts have different numeric identities
  (`992:991` on `think3`, `994:990` on `think4`), so those accounts cannot be
  used for shared workspace writes without deliberate identity migration;
- both nodes can route to and ping `172.16.0.99`, and TCP port 2049 is open;
- invoking the available NFS helper directly still returns `access denied by
  server` for `172.16.0.99:/var/nfs/shared/VibeKanban/mnt`; and
- `showmount -e 172.16.0.99` does not advertise the required Vibe Kanban
  export or authorise `172.16.100.103`/`172.16.100.104`. It advertises only
  `/volume/33482335-18e1-4928-a754-e6cea48e4ab9/.srv/.unifi-drive/Storage/.data`
  to a different client allow-list.

No persistent mount, user, service, firewall, or host configuration was
changed. The trial stopped before deployment because using a local substitute
would not validate the required missing-mount protection or shared-NFS
contract.

To resume, the storage administrator must export
`/var/nfs/shared/VibeKanban/mnt` (or provide the corrected canonical export)
to `172.16.100.103` and `172.16.100.104`. The rollout must also choose one
common workspace UID/GID that is unused on both disposable nodes, rather than
renumbering their established service accounts implicitly.
