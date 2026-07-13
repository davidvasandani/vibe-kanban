# Technical Plan: Auto-archive workspaces on terminal status (Done or Cancelled)

**Feature dir**: `specs/002-auto-archive-terminal/`
**Task**: `vk/2f63-auto-archive-wor`
**Spec**: [`spec.md`](spec.md)

## Approach

Generalise the existing `archive_workspaces_for_done_issue` in
`crates/remote/src/routes/issues.rs` into a terminal-status hook that fires on
both "Done" and "Cancelled". All plumbing already exists — this is a rename plus
a widened predicate plus a Cancelled-aware warning gate.

Grounding (real files):
- `crates/remote/src/routes/issues.rs`
  - `archive_workspaces_for_done_issue(conn, old, new)` — the function to
    generalise. Currently: status-changed guard → `ProjectStatusRepository::find_by_id`
    name == "Done" → `WorkspaceRepository::list_active_by_issue_id` → (unmerged-PR
    `tracing::warn!`) → `WorkspaceRepository::archive_active_by_issue_id`.
  - `update_issue` (~line "archive_workspaces_for_done_issue(&mut tx, …)") and
    `bulk_update_issues` (same call inside the per-item loop) — the two call
    sites, both already inside `begin_tx` … `get_txid` … `commit`.
- `crates/remote/src/db/project_statuses.rs` — `find_by_id`, `DEFAULT_STATUSES`
  (confirms "Done"/"Cancelled" are built-in names).
- `crates/remote/src/db/workspaces.rs` — `list_active_by_issue_id`,
  `archive_active_by_issue_id` (both executor-generic; run on the tx conn).
- `crates/remote/src/db/pull_requests.rs` — `list_by_issue` (for the Done warning).

## Steps

1. Add pure helper `terminal_status_name(name: &str) -> Option<&'static str>`
   ("Done" → `Some("Done")`; "Cancelled"/"Canceled" → `Some("Cancelled")`; else
   `None`; all case-insensitive).
2. Rename `archive_workspaces_for_done_issue` → `archive_workspaces_for_terminal_issue`;
   replace the hard-coded `name == "Done"` check with `terminal_status_name`; bind
   the matched `terminal` label; gate the unmerged-PR warning behind
   `terminal == "Done"`; add an info log carrying the status label + workspace
   count; update the doc comment.
3. Update both call sites to the new name (no other change — they already run it
   in-tx before `get_txid`).
4. Add `#[cfg(test)] mod tests` exercising `terminal_status_name`.

## Data model

No change. `issues.status_id → project_statuses(id)`; `workspaces.archived: bool`.
No migration, no new column, no `project_statuses` category flag (per spec Out of
Scope and Constitution V — identify terminal statuses by name as the code already
does). See [`data-model.md`](data-model.md).

## Contracts

No API/contract change. `PUT/POST /issues/:id` and `/issues/bulk` keep the same
request/response shapes; the only observable difference is that a status change
into Cancelled now also flips `archived` on the issue's workspaces within the
returned `txid`. No generated-type regeneration needed.

## Constitution check

- **I Clarity** — a named helper + a `terminal` label read more clearly than a
  bare string compare. ✅
- **II Test the contract** — unit test on `terminal_status_name`; acceptance
  criteria enumerated in the spec. (Full DB-integration of the archive path needs
  Postgres and is out of unit scope; the pure decision logic is what regresses.) ✅
- **III Small, reversible steps** — one file, generalises rather than duplicates;
  rollback = revert. ✅
- **V Remote mutations are transactional/txid-covered** — the side effect stays
  on the caller's `&mut PgConnection`, inside the existing update transaction,
  before `get_txid`; terminal statuses matched by name. ✅
- **VI Don't rebuild what shipped** — extends the existing Done hook and reuses
  `list_active_by_issue_id` / `archive_active_by_issue_id`. ✅

## Risks

Low. "Done" path is preserved call-for-call. Only new surface is the Cancelled
branch and the warning gate. Name-matching inherits the existing limitation
(a project that renames "Done"/"Cancelled" won't trigger) — unchanged from today
and explicitly out of scope.
