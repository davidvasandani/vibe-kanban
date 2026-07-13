# Implementation Plan: Auto-archive workspaces on Done **or Cancelled** (task vk/2f63-auto-archive-wor)

Single-file backend change in `crates/remote/src/routes/issues.rs`. It
generalises the existing "archive on Done" hook to a "terminal status" hook that
also covers "Cancelled". No migration, API, generated-type, or frontend change.

## Steps

1. **Add helper `terminal_status_name(name: &str) -> Option<&'static str>`.**
   - `"Done"` (case-insensitive) → `Some("Done")`.
   - `"Cancelled"` / `"Canceled"` (case-insensitive) → `Some("Cancelled")`.
   - anything else → `None`.
   Pure function, no I/O, unit-testable.

2. **Rename `archive_workspaces_for_done_issue` → `archive_workspaces_for_terminal_issue`**
   and rework its body:
   - keep the `old_issue.status_id == new_issue.status_id` early-return;
   - load the new status via `ProjectStatusRepository::find_by_id(&mut *conn, …)`;
   - `let Some(terminal) = status.and_then(|s| terminal_status_name(&s.name)) else { return Ok(()) };`
   - list active workspaces (`list_active_by_issue_id`); early-return if empty;
   - gate the existing unmerged-PR `tracing::warn!` behind `terminal == "Done"`;
   - `archive_active_by_issue_id(&mut *conn, new_issue.id)`;
   - add a `tracing::info!(issue_id, status = terminal, workspace_count, …)` line.
   - update the doc-comment to say "Done or Cancelled" and explain the
     Done-only warning.

3. **Update both call sites** (`update_issue`, `bulk_update_issues`) to call the
   renamed function. No other change — they already run it inside the tx before
   `get_txid`.

4. **Add `#[cfg(test)] mod tests`** at the bottom of the file with a
   `terminal_status_name` table test (Done/done/DONE, Cancelled/cancelled/Canceled,
   negatives: "In progress", "Backlog", "To do", "").

## Validation

- `cargo test -p remote` → new unit test green.
- `cargo clippy -p remote` / `pnpm run backend:check` → clean.
- `pnpm run format`.
- Logical trace of the tx path: status change into Cancelled archives active
  workspaces atomically with the status write (same txid); no-op transitions and
  non-terminal statuses do nothing.

## Risk / rollback

Low: additive to a single, already-tested code path; "Done" behaviour is byte-for
-byte preserved (same list/warn/archive calls). Rollback = revert the one file.
