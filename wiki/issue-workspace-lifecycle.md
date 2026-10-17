# Issue and workspace lifecycle

Local execution startup already unarchives SQLite workspaces except for archive
scripts. That local write alone does not update the remote issue board. Route
accepted activity through `ContainerService::activate_workspace`; the local
implementation persists it and schedules the existing best-effort remote sync.
Queue acceptance must publish the queued message **before** awaiting activation:
a turn can finish during I/O, and its finalizer needs to see the pending message.
Never put remote HTTP latency ahead of queue insertion or execution dispatch.

The remote workspace PATCH is the shared convergence boundary for manual and
automatic activation. `WorkspaceRepository::update` locks the linked issue before
the workspace (matching terminal-issue archival), observes the persisted archive
state, and commits reactivation plus conditional issue reopening in one Postgres
transaction. Only an actual archived-to-active transition reopens Done; metadata
updates and repeated active snapshots do not. Status matching follows the existing
case-insensitive, project-scoped name convention. Cancelled/custom statuses and
projects without In progress remain unchanged.

The legacy workspace endpoint returns a Workspace directly, unlike newer txid
mutation envelopes. Preserve that API contract when adding transactional effects.
SQLite and Postgres remain separate stores: synchronization errors are logged,
with existing post-login reconciliation; this is not a distributed transaction
or a new durable retry queue.

Completion-time sync must read current workspace metadata instead of sending the
archive flag captured in an earlier execution context. A stale snapshot can undo
a later user lifecycle decision.

Regression seams: remote `db::workspaces::lifecycle_tests` uses isolated Postgres
databases for transitions, rollback and concurrency; services
`remote_sync::lifecycle_tests` checks local persistence and actual HTTP unarchive
requests, including an already-active local workspace needing remote reconciliation.

## Contributed by

- `vk/e464-issue-and-worksp`
