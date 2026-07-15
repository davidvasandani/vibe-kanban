# Prior Knowledge: Remote/Local Workspace Archive Consistency

Searched both project knowledge indexes (`docs/knowledge-base/INDEX.md` and
`wiki/INDEX.md`) and their relevant topic pages for workspace archival,
terminal issue side effects, remote/local synchronization, and Electric data.

## Directly relevant: terminal-status side effects

`docs/knowledge-base/issue-status-side-effects.md` (task
`2f63-auto-archive-wor`) documents the existing remote behavior:

- `archive_workspaces_for_terminal_issue` is called by both single and bulk
  issue-update paths in `crates/remote/src/routes/issues.rs`.
- It archives remote PostgreSQL workspace rows on the issue mutation's existing
  transaction, preserving atomicity and the Electric txid handshake.
- Terminal matching is name-based (`Done`, `Cancelled`, and `Canceled`, case
  insensitive), guarded on an actual status change.
- The page explicitly records the current boundary: the behavior is remote-only
  and the local SQLite workspace path has no equivalent hook.
- Reopening does not automatically unarchive workspaces.

Implication for this task: preserve the transaction and terminal-status logic;
the defect is the documented remote/local boundary, not a failure to archive the
remote row. The new behavior should add convergence after the remote row becomes
visible rather than moving the existing side effect out of its transaction.

## Relevant: Electric shape delivery

`wiki/electric-sync-fallback.md` (task `vk/a96d-electric-sync-er`) documents
that shared web providers consume remote rows Electric-first with a REST polling
fallback. A shape can update live, and fallback snapshots can replace the full
collection. Collection configuration is cached and callbacks must be stable.

Implication for this task: reconciliation should be derived idempotently from
the current remote workspace snapshot, not rely on receiving exactly one
Electric event. That works for live Electric updates, fallback snapshots,
provider remounts, and reconnects.

## Supporting UI architecture

`wiki/kanban-issue-panel-sections.md` confirms the kanban issue panel is shared
between local and remote frontends and that data is supplied by web-core
containers/providers. The visual issue panel does not own synchronization
behavior.

Implication for this task: implement reconciliation in shared provider/hook
logic, not in the presentational workspace card or a single click/drag handler.
This also covers single and bulk issue mutations and updates initiated by other
clients once the remote workspace shape changes.

## Constraints carried into planning

1. Keep the remote issue mutation and workspace archive atomic and txid-covered.
2. Treat remote workspace snapshots as level-triggered state; reconciliation
   must be idempotent and safe after reconnect/remount.
3. Use the existing local workspace update endpoint so all established local
   archive lifecycle behavior remains centralized.
4. Preserve the existing no-auto-unarchive policy.
