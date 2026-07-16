# Terminal-status side effects on the remote issue-update path

**Contributing tasks:** `2f63-auto-archive-wor`, `f464-vk-workspace-mgm`

How the remote/cloud server reacts to an issue changing status — specifically the
"archive an issue's workspaces when it reaches a terminal status" behaviour — and
the conventions any similar status-triggered side effect must follow.

## Where it lives

`crates/remote/src/routes/issues.rs`. The single hook
`archive_workspaces_for_terminal_issue(conn, old_issue, new_issue)` is called from
**both** mutation paths, `update_issue` and `bulk_update_issues`, and nowhere
else. Both callers already:

1. `begin_tx` → `IssueRepository::update(&mut *tx, …)`
2. run the side-effect hook **on the same `&mut *tx`**
3. `get_txid(&mut *tx)` → `tx.commit()`

So the side effect commits atomically with the status write and is covered by the
one `txid` the client waits on over the ElectricSQL stream. This is the load-bearing
rule (constitution: "Remote mutations are transactional and txid-covered"): a
status-triggered side effect that opened its own transaction/pool connection could
land after the client already dropped optimistic state → visible flicker or a
missed update. Keep it on the caller's connection.

## The decision logic

- **Guard on actual change:** `if old_issue.status_id == new_issue.status_id { return Ok(()) }`.
  Issue updates frequently re-save unchanged status; don't re-run the effect.
- **Identify terminal statuses by name, case-insensitively.** `project_statuses`
  are per-project, user-customisable, and have **no** terminal/category column
  (see `db/project_statuses.rs`, `DEFAULT_STATUSES` — "Done"/"Cancelled" are just
  default names). The helper `terminal_status_name(&str) -> Option<&'static str>`
  returns the canonical label for "Done" and for "Cancelled"/"Canceled". Known
  limitation: a project that *renames* Done/Cancelled won't trigger — accepted,
  and identical to how the original Done-only hook behaved.
- **Early-out on empty:** `WorkspaceRepository::list_active_by_issue_id` then
  return if empty; `archive_active_by_issue_id(issue_id)` flips `archived = TRUE`
  for that issue's active workspaces (both executor-generic → run in-tx).

## Warning vs. failure discipline (copy this for similar effects)

- A failure loading status / listing workspaces / performing the archive is
  `map_err`'d to a 500 → **fails the whole update transaction** (no partial state).
- A failure only while gathering *advisory* data (here: pull requests, to decide
  whether to log the "PRs not all merged" warning) uses `unwrap_or_else(Vec::new)`
  → **degrades to no-warning, never fails the request.**
- The unmerged-PR warning is **Done-only**: cancelled work is intentionally
  abandoned, so the warning would be noise. Note PRs are linked to issues, not
  reliably to individual workspaces (`pull_requests.workspace_id` isn't populated
  on creation), so the merge check is issue-level.

## Remote-to-local reconciliation

The remote transaction only archives the cloud `workspaces` row. A linked local
workspace lives in the host's SQLite database and must be reconciled separately.
`ProjectProvider` combines its remote workspace shape snapshot with the active and
archived workspace streams exposed by `WorkspaceContext`, then
`useRemoteLocalArchiveReconciliation` archives each local workspace that is still
active while its linked remote row is archived.

This is deliberately **level-triggered**, not tied to the status-change event. The
same comparison runs after Electric updates, fallback snapshots, reconnects, and
provider remounts, so temporary disconnection does not lose the side effect. It
uses the existing `workspacesApi.update(id, { archived: true })` path rather than a
second local mutation mechanism. The reconciliation helper:

- selects unique `local_workspace_id` values from archived remote rows that still
  appear in the local active set;
- ignores unlinked or remote-only rows and local workspaces already archived;
- deduplicates requests while each local archive is in flight;
- isolates failures per workspace and allows a later snapshot to retry; and
- never auto-unarchives when a remote issue or workspace is reopened.

`ProjectProvider` can also be mounted by dialogs outside `WorkspaceProvider`, so it
must read `WorkspaceContext` as optional and skip reconciliation when local
workspace streams are unavailable. Do not replace that with the throwing
`useWorkspaceContext` hook unless every provider composition is changed first.

Archived local workspaces enter the accelerated (1h) worktree-cleanup window vs.
the standard 72h — see `crates/db` cleanup queries.

## Not covered

- Un-archiving on reopen (terminal → non-terminal) — not implemented either side;
  reconciliation is intentionally archive-only.
- The local SQLite `Task` path (`crates/db`) has no equivalent hook; this
  behaviour is remote-only.
