# Prior knowledge — vk/4dac-filter-workspace

Sources searched (read-only): `wiki/INDEX.md` and its pages, and
`docs/knowledge-base/INDEX.md` and its pages. Search terms: sidebar, filter,
issue status, scratch / UI preferences, Electric shapes, i18n. No page covers
the workspace sidebar filter dialog directly. The matches below are the
closest.

## What applies

- **Statuses have no category, so match by name, case-insensitively.**
  `docs/knowledge-base/issue-status-side-effects.md` and
  `wiki/kanban-items-state-and-activity-grouping.md` both say
  `project_statuses` are per project and user-customisable, with no
  terminal or category column. Existing code identifies "Done" / "In
  progress" by lower-cased name. The global UI preference
  `list_view_status_filter_name` is stored **by name, not id**, for the same
  reason (see its doc comment in `crates/db/src/models/scratch.rs`).
  → Store the hidden statuses as names and compare them trimmed and
  lower-cased.
- **Issue identity comes from the remote workspace record.**
  `wiki/issue-workspace-advisory.md`: join local sidebar records to remote
  workspaces through `local_workspace_id`, and read `project_id` / `issue_id`
  from the remote row. Never match by name.
  → `useUserContext().workspaces` already provides this. The sidebar already
  builds `remoteProjectByLocalId` from it.
- **Electric collections are cached per source.**
  `wiki/electric-sync-fallback.md`: `createShapeCollection` caches by
  collection id. `useAllOrganizationProjects.ts` shows the pattern for
  subscribing to N shapes without calling `useShape` in a loop
  (`createShapeCollection` + `subscribeChanges`). Its `config` is optional, so
  errors don't reach the banner from there. That is acceptable for an
  enrichment-only feature.
  → Use the same pattern for per-project issues and statuses. The kanban
  board and `LinkedIssueProvider` already use these shapes, so the cache is
  often warm.
- **Missing enrichment is not evidence.**
  `docs/knowledge-base/workspace-summary-ordering.md`: projections must stay
  useful with only the base record. Apply one shared pipeline to both the
  active and the archived list, before pagination.
  → Fail open. A workspace whose issue or status hasn't loaded stays visible.
  Run a single pure filter function for both lists.
- **Curating filters vs explicit lookups.** `wiki/kanban-board-filtering.md`
  says view *defaults* should yield to search, while deliberate filters still
  apply. This status filter is a deliberate, user-set filter, so it keeps
  applying during search, like the project and PR filters. It also says to
  pair every absence assertion in a test with a positive case.
- **i18n.** `wiki/issue-workspace-advisory.md` and
  `docs/knowledge-base/locale-key-consistency.md`: every new key must exist in
  every locale (`scripts/check-i18n.sh`), and interpolation identifiers must
  match byte for byte.
- **Formatting prerequisite.**
  `docs/knowledge-base/worktree-formatting-prerequisites.md`: run
  `pnpm install --frozen-lockfile` before `pnpm run format` in a fresh
  worktree.
- **Scratch round trip.** From the code (no wiki page): `UiPreferencesData`
  is a typed Rust struct, and `WorkspaceFilterStateData` has no
  `serde(flatten)` catch-all. A frontend-only field would be dropped by the
  server. The field needs `#[serde(default)]` plus `pnpm run generate-types`.
