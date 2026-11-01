# Issue and workspace lifecycle

Local execution startup already unarchives SQLite workspaces except for archive
scripts. That local write alone does not update the remote issue board. Route
accepted activity through `ContainerService::activate_workspace`; the local
implementation persists it and schedules the existing best-effort remote sync
when no unacknowledged completion intent is held. Activity is not an explicit
withdrawal of completion; a timer must not reopen Done.
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

## Automatic archive is separate from manual archive

See `docs/knowledge-base/issue-status-side-effects.md` for the local automatic
admission rule. Never use the manual workspace update endpoint for remote
archive reconciliation. A blocked local archive retains remote intent for a
later retry; remote unarchive can reopen Done, so it is not a harmless repair.
Pending wake-up status comes from its delivery ledger, not its deadline alone.

## Durable archive intent and explicit override (VAS-910)

Remote `workspace_archive_events` assigns a stable event UUID to archived
inserts and false-to-true transitions. Terminal issue transitions also refresh
the event for already-archived linked workspaces. Metadata writes do not refresh
it. Record this in the issue transaction; do not use ordinary updated_at as an
event identity. The authenticated archive-intent endpoint returns the current
event only while remote archive is requested.

SQLite `workspace_archive_intents` records rule, event time, archive time, and
acknowledgement. Explicit unarchive atomically acknowledges known events and
records a cutoff for older events that arrive later. Automatic admission and
scheduling share immediate transactions. Archive timestamps belong only to the
eligible event actually admitted, not every pending source.

The PR monitor retries persisted intent after holds clear. Revalidate remote
events before admission: acknowledge withdrawn events, and register a replacement
before retiring the old event so retry selection cannot lose the workspace.
Remote outages defer remote events but do not block an eligible local merge.
Open PRs still block PR-merge intent. Provenance is available from the local
archive/provenance endpoint, MCP list_workspaces, and the workspace sidebar.

## Contributed by

- `vk/e464-issue-and-worksp`

- `VAS-892` (`vk/1e1c-don-t-auto-archi`)

- VAS-910 (`vk/e8d6-wake-ups-an-auto`)
