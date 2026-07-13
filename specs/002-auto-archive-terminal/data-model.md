# Data Model — Auto-archive on terminal status

No schema change. Relevant existing entities (remote / Postgres):

## `issues`
- `id: uuid`, `project_id: uuid`, `status_id: uuid → project_statuses(id)`, …
- Status transitions are writes on the REST update path
  (`routes/issues.rs::update_issue` / `bulk_update_issues`), each inside one
  transaction returning a `txid`.

## `project_statuses`
- `id`, `project_id`, `name`, `color`, `sort_order`, `hidden`, `created_at`.
- Per-project, user-customisable. **No terminal/category column.**
- Built-in defaults (`db/project_statuses.rs::DEFAULT_STATUSES`) include
  `"Done"` and `"Cancelled"`. Terminal statuses are identified by **name**
  (case-insensitive), matching the existing "Done" hook.

## `workspaces`
- `id`, `project_id`, `owner_user_id`, `issue_id: uuid?`, `archived: bool`, …
- `archived` streams to clients over the ElectricSQL workspace shape; the client
  renders archived vs active from it (no client-side archive logic).
- Helpers used: `list_active_by_issue_id(issue_id)` (archived = FALSE),
  `archive_active_by_issue_id(issue_id)` (sets archived = TRUE for that issue's
  active rows). Both executor-generic → run on the update transaction connection.

## State transition added by this feature
`issue.status_id` changes → new status name ∈ {Done, Cancelled} (case-insensitive)
→ that issue's `workspaces` with `archived = FALSE` become `archived = TRUE`,
committed in the same transaction as the status write.
