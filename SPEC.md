# SPEC — Filter sidebar workspaces by linked issue status

Task: `vk/4dac-filter-workspace`

## Problem

The workspaces sidebar lists every active workspace. Many are linked to an
issue that has moved to a state where the operator does not need to see the
workspace any more, for example **In review**. The sidebar filter dialog
(funnel icon next to the sort icon) can narrow by **Project** and **PR**
only, so the operator has no way to hide those workspaces.

## Goal

Add an **Issue status** control to the sidebar filter dialog. The operator
toggles issue statuses off, and workspaces whose linked issue is in a hidden
status disappear from the sidebar list.

## Behaviour

1. **Hide list, not allow list.** The preference stores the statuses to
   *hide*. A status that appears later (a new column, a new project) shows by
   default, so a new status can never silently hide work.
2. **By status name, case-insensitive.** Status ids are per project, and this
   preference is global. So it stores names, the same approach as the existing
   `list_view_status_filter_name`. Hiding "In review" hides it in every
   project that has a status with that name. Names are compared trimmed and
   lower-cased, and the dedupe for the options list uses the same rule.
3. **Options.** The dropdown lists every status name from the projects that
   have linked workspaces (hidden board columns too), deduped by name and
   ordered by lowest `sort_order`. It also lists every name already hidden in
   the preference, even when no loaded project has it, so the operator can
   always un-hide it. A **No issue** option (sentinel `__no_issue__`) hides
   workspaces that have no linked issue.
4. **Fail open.** A workspace stays visible unless its issue's status is
   known and hidden. This covers a workspace with no remote record, an issue
   or status that hasn't loaded, or an issue deleted remotely. The one
   exception is **No issue**: a workspace with no linked issue (no remote
   record, or a remote record with `issue_id = null`) is hidden only when
   **No issue** is selected.
5. **Scope.** The filter applies to both the active and the archived list,
   and combines with the project filter, the PR filter and search (AND), the
   same as the existing filters. The funnel icon turns brand-coloured when
   any status is hidden, and **Clear filters** resets it.
6. **Persistence.** It is stored with the other sidebar filters in the
   UI-preferences scratch, as `workspace_filters.hidden_issue_status_names`
   (`#[serde(default)]`, so older payloads still load). The Rust scratch
   model is typed, so the field has to be added there. Otherwise the server
   would drop it on a round trip.
7. **Data cost.** Issue and status data is synced per linked project through
   the existing cached Electric shapes (`PROJECT_ISSUES_SHAPE`,
   `PROJECT_PROJECT_STATUSES_SHAPE`). The subscriptions are enabled only while
   the filter dialog is open or while at least one status is hidden. The
   default sidebar therefore opens no new shapes.

## Non-goals

- Filtering the carousel view or the kanban board.
- Per-project status selection.
- A status badge on each sidebar row.

## Acceptance

- Hiding "In review" removes workspaces whose linked issue is In review, in
  every project, from the sidebar. Un-hiding brings them back.
- Workspaces without a linked issue stay visible unless **No issue** is
  selected.
- The preference survives a reload (scratch round trip through the Rust
  type).
- Unit tests cover the pure filter and the options builder, including the
  fail-open cases and case-insensitive matching.
- `pnpm run check`, `pnpm run lint`, the web-core tests, the i18n check,
  `generate-types:check` and `cargo test -p db` pass.
