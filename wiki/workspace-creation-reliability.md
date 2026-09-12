# Workspace creation: repository ownership and failure evidence

## Align the local queue with durable ownership

Repository administration is leased by registered repository UUID in SQLite.
The coordinator-local checkout and a shared bare store have different canonical
common directories while representing that same UUID. A path-keyed local admin
mutex therefore permits both callers through, only for one to fail its durable
claim. Use repository UUID for both layers. The separate Git operation mutex
continues to protect the physical common directory.

The failure appears to the user as a generic creation failure. Coordinator logs
name the workspace UUID and the specific repository administration lock. These
logs, not the frontend wording alone, identify the failed phase.

## Wait only before side effects

Cancellation or coordinator restart can leave an unexpired lease. Returning
"lock busy" immediately makes a transient ownership condition a terminal
workspace failure. Poll the existing atomic acquisition predicate until its
owner releases or the lease expires, with a monotonic budget equal to the
configured lease duration and 100 ms sleeps. The coordinator currently uses a
five-minute lease. An in-flight database call may exceed the polling budget;
database errors still return immediately rather than being retried as contention.

Never force-release the prior owner, discard fencing tombstones, or retry the
workspace workflow. Each attempt uses a fresh current time for expiry while
retaining its operation ID. Once acquired, the existing generation/operation
checks protect release. A cancelled queued caller releases its local mutex.

Do not add unconditional release in guard Drop: blocking Git work may continue
after its async waiter is cancelled. Waiting at admission does not claim to make
all Git operations cancellation-safe or renew an already-held lease.

Regression coverage should include distinct paths with one repo ID, durable
contention followed by release, retained fencing against an old owner,
abandoned-lease expiry, bounded refusal of a live lease, cancellation while
queued, unrelated IDs, and immediate propagation of database errors. The
contention regressions must fail against the old implementation.

## Distinguish an ownership rejection from a database error

"Workspace provisioning completed but ready state could not be persisted" is
emitted when the final Provisioning-to-Ready compare-and-set changes zero rows.
It is not itself proof of a SQL error. A deleted workspace or competing placement
mutation can cause it. Inspect the active database and correlate the workspace
UUID before changing transition rules; missing historical rows cannot establish
which event happened. Never respond by replaying already-completed provisioning.

On the self-hosted coordinator, the active database is `db.v2.sqlite`; retained
`db.sqlite` files belong to older schemas. Confirm the active file through the
service's file descriptors rather than assuming the first matching filename is
current.

## Contributed by

- `vk/40fb-workspace-creati`
