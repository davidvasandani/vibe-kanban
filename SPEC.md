# Technical Spec: Auto-archive workspaces when an issue is marked Done **or Cancelled**

> Task `vk/2f63-auto-archive-wor`. Full SpecKit artifacts live in
> `specs/002-auto-archive-terminal/` (`spec.md`, `plan.md`, `tasks.md`,
> `analyze.md`) and the constitution in `.specify/memory/constitution.md`.
> This file is the repo-root technical summary required by the task pipeline.

## Context — what already ships

The remote/cloud server already archives an issue's still-active workspaces
when the issue is moved into the **"Done"** status. It was added in upstream
commit `3510c588` ("Archive workspaces when an issue is manually marked Done")
and lives entirely server-side in
`crates/remote/src/routes/issues.rs::archive_workspaces_for_done_issue`, invoked
from both `update_issue` and `bulk_update_issues` **inside the same Postgres
transaction** as the status write, so the archive is covered by the `txid` the
client waits on. The frontend has no archive-on-status logic; it simply renders
the `archived` flag streamed over the ElectricSQL workspace shape.

Data model (remote / Postgres):
- `issues.status_id` → `project_statuses(id)`. Statuses are per-project and
  user-customisable; the built-in defaults (`db/project_statuses.rs::DEFAULT_STATUSES`)
  include `"Done"` and `"Cancelled"`. There is **no** category/terminal flag on
  the row — the existing code matches by status **name** (`eq_ignore_ascii_case("Done")`).
- `workspaces.archived: bool`; helpers `list_active_by_issue_id` /
  `archive_active_by_issue_id` already exist and are executor-generic (run inside
  a tx).

## The gap

Cancelling an issue leaves its workspaces active — they keep consuming a
sidebar slot and the standard 72h worktree-cleanup window instead of the
accelerated archived (1h) window. Users expect a cancelled issue to behave like
a done one: its workspaces should be archived automatically.

## Solution

Generalise the existing hook to fire on **either** terminal status, keeping the
name-match approach already established for "Done" (consistent, no schema
change, respects user-renamed statuses only insofar as the current feature does).

In `crates/remote/src/routes/issues.rs`:

1. Add a small pure helper
   `fn terminal_status_name(name: &str) -> Option<&'static str>` returning
   `Some("Done")` / `Some("Cancelled")` (case-insensitive; also accepts the
   American `"Canceled"` spelling, normalised to `"Cancelled"`) else `None`.
2. Rename `archive_workspaces_for_done_issue` →
   `archive_workspaces_for_terminal_issue`. It:
   - returns early if `status_id` did not change;
   - loads the new status, returns early unless `terminal_status_name` matches;
   - lists active workspaces, returns early if none;
   - **only for "Done"** emits the existing "PRs not all merged" warning
     (cancelled work is intentionally abandoned, so the warning would be noise);
   - archives via `archive_active_by_issue_id`;
   - logs an info line with the status name and workspace count.
3. Update the two call sites (`update_issue`, `bulk_update_issues`) to the new
   name. Behaviour for "Done" is unchanged; "Cancelled" is newly covered.

Add a `#[cfg(test)]` unit test for `terminal_status_name` (no DB needed):
Done/done/DONE → `Some("Done")`, Cancelled/cancelled/Canceled → `Some("Cancelled")`,
"In progress"/"Backlog"/"To do"/"" → `None`.

## Scope

- **Remote crate only**, mirroring where the "Done" feature lives. No local
  (`crates/db` SQLite `Task`) change — that path has no equivalent archive hook.
- No migration, no new API surface, no generated-type change, no frontend change
  (the `archived` flag already streams to clients).
- Terminal set is `{Done, Cancelled}` matched by name, exactly like the existing
  "Done" match; no new "status category" concept is introduced.

## Validation

- `cargo test -p remote` (new `terminal_status_name` unit test passes).
- `pnpm run backend:check` / `cargo clippy` on `crates/remote` clean.
- `pnpm run format` applied.
- Manual/logical trace: moving an issue Todo→Cancelled archives its active
  workspaces in the same txid; Todo→In-progress does not; Done→Done (no change)
  does not re-run.

## Files

- `crates/remote/src/routes/issues.rs` (edited)
