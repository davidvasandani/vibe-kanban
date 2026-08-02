# Clustered workspace execution and shared-storage safety

Tags: `957e-clustered-vibe-k`, `19a4-git-worktrees-br`

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

Single-writer applies to *administration*, not to writes. A linked worktree
keeps its `index`, `HEAD`, `ORIG_HEAD` and `logs/` inside the repository's
`worktrees/<n>/` directory, so every worker Git command — `git status` included
— writes into shared storage. Design the ownership model for concurrent
multi-node writes: create the store `core.sharedRepository=group` **at clone
time** (setting it afterwards leaves every object and directory git already
created at the cloning process's umask), and disable automatic maintenance
before the first worktree is registered. `git gc --auto` fires opportunistically
on ordinary commands and prunes worktrees, so without `gc.auto=0` and
`gc.worktreePruneExpire=never` a routine `git status` on a worker can unregister
a different workspace.

Cleanup must require positive evidence that no execution is active. An offline
or unreachable worker means the workspace is indeterminate, not idle, so retain
the files until reconciliation or operator intervention proves reclamation is
safe.

## A worktree is only as portable as the repository behind it

`git worktree add` records absolute paths in **both** directions: the worktree's
`.git` file names an administration directory inside the repository, and that
directory's `gitdir` file names the worktree's `.git` back. Creating a worktree
on shared storage does not make it shared — the repository it points at must
also resolve, at the same absolute path, on every node.

Creating cluster worktrees from a coordinator-local checkout therefore produces
workspaces where every Git command fails with `fatal: not a git repository:
(null)`, and it fails silently: provisioning reports ready, the mount is
healthy, and the agent only finds out several tool calls into its first turn.
Give each repository a bare store under the shared root at a location every node
derives from its id, and create cluster worktrees from that.

Four rules make the property hold rather than merely documenting it:

- **Assert structure, not spelling.** Check that the resolved common directory
  is *inside* the shared root. Never check that a path does not contain a
  known-bad prefix.
- **Existence proves nothing.** A same-named local directory is not the
  repository. On these hosts `/srv/src/<repo>` exists on workers too, holding a
  different clone — a resolver that accepts it binds the workspace to unrelated
  history.
- **Prove the objects.** `git rev-parse` echoes any well-formed 40-hex string
  whether or not the repository holds it. Use `git cat-file -e <rev>^{commit}`
  before treating a branch as present.
- **Check level-triggered.** A check that runs only where the worktree is
  created is an edge trigger. Re-probe at startup, at placement, and before
  dispatch.

Consolidating every workspace of a repository into one store re-scopes existing
cleanup: a repository-wide `git worktree prune` that was safe when a checkout
held a handful of registrations now reaches every workspace of that repository,
on every node — and prune decides by asking whether a worktree directory is
present, which on a network mount is indistinguishable from unreadable. Scope
cleanup to the worktree being cleaned. When a namespace is consolidated,
re-derive the blast radius of everything that touches it rather than inheriting
the old conclusion.

## Repair a broken worktree; never recreate it

A worktree with a dangling pointer still holds the agent's edits, its untracked
build output, and any commits made before the breakage. Re-link it in place:
write `worktrees/<n>/{commondir,gitdir,HEAD}` and the worktree's `.git`, run
`git worktree repair`, then `git reset` to rebuild the absent index — without
that last step the worktree reports every tracked file as simultaneously deleted
and untracked. No working-tree file is touched.

Two sequencing rules matter. The usual "capture state before mutating" order is
impossible here, because `git status` fails until the pointer is fixed; invert
it deliberately — repair pointers (non-destructive), then capture, then consider
anything that could lose work. And repair must run *before* the ordinary
"ensure the workspace exists" path, not after: that path sees a branch missing
from the new store, fails to repair linkage it cannot resolve, and falls through
to destructive recreation, deleting exactly what the repair existed to save.

Refuse rather than guess. Refuse when the branch's commits cannot be proven
present, when the branch is checked out by another worktree, or when the linkage
is merely *indeterminate* — unknown is not broken, and repairing on a guess
points a live workspace at history that is not its own.

## The worker refuses; it does not repair

The worker is the only participant that can tell whether a worktree resolves on
the node that will run the work, and the wrong one to fix it. Probe at dispatch
admission and reject with a specific reason, terminalising the job.

The probe is pure filesystem reads, which is what lets it live in a crate the
worker already depends on rather than pulling Git into it. Two details decide
whether it helps or hurts:

- Enumerate directories only. `<file>/.git` stats as `NotADirectory`, an error
  rather than "absent", and a workspace root routinely holds `CLAUDE.md`,
  `AGENTS.md` and copied attachments. Treating that error as fatal refuses every
  dispatch to every workspace.
- Skip `.recovered-<epoch>` siblings. They are preserved evidence of an earlier
  rescue whose registration is deliberately gone; refusing over one strands the
  workspace permanently, since the worker cannot repair it.

Distinguish *not applicable* from *broken*, and never satisfy a dangling pointer
with a local directory that happens to match.

## Verification pattern

Cover protocol signatures, duplicate dispatch, ordered replay, cancellation
escalation, mount identity, scheduler exclusions, host-aware previews, and Nix
role evaluation with focused tests. Then run a two-node deployment exercise
that disconnects the coordinator, cancels a process group, removes the shared
mount, and verifies worktree integrity. Passing local tests does not replace
that deployment gate.
