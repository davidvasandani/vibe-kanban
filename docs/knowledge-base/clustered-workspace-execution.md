# Clustered workspace execution and shared-storage safety

Tags: `957e-clustered-vibe-k`

## Keep authority central and process ownership local

The coordinator remains authoritative for SQLite records, workspace placement,
Git worktree administration, approvals, and user-facing execution state. A
worker owns only the processes assigned to its sticky workspace: spawning,
ordered event delivery, cancellation, terminal sessions, and preview traffic.

Persist the worker ID on both the workspace and execution job. Never infer
affinity from the currently selected UI host, and never retry a dispatch on a
different worker. Dispatch is idempotent by coordinator execution ID so a lost
response cannot start a duplicate agent.

## Treat a shared mount as a capability, not a directory

An existing path does not prove that NFS is mounted. Before becoming
schedulable, each worker verifies all of the following:

- the path is a mount point backed by the expected NFS export;
- a coordinator-issued probe is visible;
- required directories are writable;
- storage-side ownership matches the configured expected UID and GID;
- capacity remains below the operational threshold.

Mount loss immediately makes the worker unschedulable. Preserve workspace data
and report uncertainty; do not fall back to an identically named local
directory.

Keep the NFS mountpoint separate from the application root. Some managed NAS
exports retain one owner on the export root but map all new collaborative
writes to another UID/GID. Mount the export at a stable parent, create a
dedicated cluster child through the coordinator, and validate that child's
mapped identity. Do not conflate the worker's local account UID/GID with the
storage-side identity produced by NFS squashing.

Deployment credentials must remain runtime paths. Nix module options for
private keys should accept absolute strings, reject `/nix/store/` paths, and
load them through systemd credentials. A Nix path literal can copy a secret
into the world-readable store.

## Make event replay monotonic

Workers append execution events to a bounded journal with monotonically
increasing sequence numbers. The coordinator acknowledges the last persisted
sequence and reconnects from that cursor. It ignores duplicates and rejects
gaps instead of inventing completion.

On restart, reconcile both directions:

- worker jobs absent from SQLite are quarantined or terminated by policy;
- SQLite jobs marked running but absent from the worker become interrupted or
  indeterminate;
- persistent jobs may be re-adopted only with verifiable worker evidence;
- ordinary agents are never silently marked complete after a disconnect.

Lease expiry also needs to reach user-facing reads. Expiring stale `online`
rows only inside scheduler selection leaves an admin UI claiming a dead worker
is healthy; expire leases before listing workers (or in a periodic registry
task) as well as filtering them during placement.

A failed dispatch must also terminalise its worker-job record. Otherwise a job
that never started appears pending indefinitely and contaminates later
reconciliation.

## Bind authentication to the complete request

Worker requests are signed over timestamp, HTTP method, the full path and query,
and a digest of the exact body bytes. Verifying only metadata authenticates the
caller but permits body substitution. Omitting the query permits replay against
a different event cursor or preview target.

Apply an explicit body limit before buffering signed requests. Account for
encoding expansion: a base64-wrapped preview body is larger than the underlying
payload.

Framework nesting can rewrite the URI visible to inner middleware. In Axum,
verify worker signatures against `OriginalUri` so a request signed as
`/api/workers/...` is not checked as the stripped `/workers/...` target.

Anti-replay nonces and idempotent dispatch retries must be designed together.
On a transient dispatch retry, preserve the execution ID and request digest but
refresh the authority timestamp and nonce. Replaying the exact envelope should
remain forbidden, while the refreshed envelope returns the existing worker job.

## Preserve affinity through browser subrequests

Preview routing needs workspace ID, execution ID, and generation. Query
parameters on the initial iframe URL are insufficient because relative assets
and WebSocket connections do not inherit them. Encode the routing tuple in the
preview hostname (or another browser-sticky authority component), then resolve
every HTTP and WebSocket request from that identity. Forward and echo the
selected WebSocket subprotocol.

## Keep shared Git administration single-writer

Workers may run ordinary Git commands inside their assigned worktree, but only
the coordinator may add, remove, prune, or reclaim worktrees and delete shared
branches. Serialize these operations per repository with fenced ownership; a
plain lock file cannot distinguish a live owner from a stale one.

Cleanup must require positive evidence that no execution is active. An offline
or unreachable worker means the workspace is indeterminate, not idle, so retain
the files until reconciliation or operator intervention proves reclamation is
safe.

## Verification pattern

Cover protocol signatures, duplicate dispatch, ordered replay, cancellation
escalation, mount identity, scheduler exclusions, host-aware previews, and Nix
role evaluation with focused tests. Then run a two-node deployment exercise
that disconnects the coordinator, cancels a process group, removes the shared
mount, and verifies worktree integrity. Passing local tests does not replace
that deployment gate.
