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

The UI.com administrator then added both clients with read-write access to the
`VibeKanban` shared drive. The resulting canonical export is
`172.16.0.99:/var/nfs/shared/VibeKanban` over NFSv3 (the earlier `/mnt` suffix
was invalid). Read-only NFSv3 mounts succeeded from both nodes and reported
the shared root owner as UID/GID `988:988`.

## 2026-07-31 — Authorised think3/think4 live result

The disposable trial completed using `think3` as coordinator and `think4` as
worker. Both nodes mounted the canonical export at
`/srv/vibe-kanban-shared`. UniFi Drive's Collaborative squash maps new writes
to storage identity `977:988`, while the export root retains its pre-existing
`988:988` ownership. The safe layout therefore uses the coordinator-created
`/srv/vibe-kanban-shared/cluster` child as the application root. Switching the
share to Isolated mode was explicitly rejected because UI.com warns that the
change is permanent and disables UniFi web/SMB management for the drive.

Live evidence:

- the worker registered over the authenticated LAN protocol and reported
  `mount_status=healthy` after observing the coordinator probe through NFS;
- nested Axum routing initially caused worker signatures to be checked against
  `/workers/...` instead of the signed `/api/workers/...` target; preserving
  `OriginalUri` fixed registration and has a regression test;
- execution `6af47bf8-4b64-412a-aaad-0f9c1079c4e4` produced five contiguous
  ordered events, and replay after completion returned `alpha` then `omega`;
- a same-execution/same-digest redispatch returned the existing worker job,
  while a changed digest returned a conflict;
- execution `fe181fd7-292f-48f0-b4a2-e435923374f5` was terminated as
  `Killed`; repeating cancellation returned `AlreadyTerminal`;
- execution `519dc00e-780d-4e20-b4a1-2fc877f5edbd` was running when the worker
  service stopped and recovered from the durable journal as `Interrupted`,
  never `Completed`; and
- stopping heartbeats exposed that the admin list did not expire a stale
  `online` row. The list path now expires leases before returning workers, so
  UI-visible health agrees with scheduler eligibility.

The trial also demonstrated that an exact request-envelope replay is rejected
by nonce protection. Dispatch transport retry now refreshes only the authority
timestamp and nonce while retaining the execution ID and request digest, so
idempotency and anti-replay requirements coexist.

Deployment changes derived from the trial separate the NFS mountpoint from the
application root and separate local worker account UID/GID from the expected
storage-side UID/GID. The homelab evaluation asserts NFSv3, mount ordering, the
prepared cluster child, and UniFi's observed `977:988` mapped identity.
