# Research: Filter sidebar workspaces by issue status

## D1: Key the preference by status name, not id
- **Decision:** store normalized status names.
- **Why:** `project_statuses` rows are per project and carry no category
  (`docs/knowledge-base/issue-status-side-effects.md`). The preference is
  global and must span projects. There is already a precedent:
  `UiPreferencesData.list_view_status_filter_name` ("stored by status name
  (not id, since status ids are per-project and this preference is global)").
- **Rejected:** a per-project map of status ids. It is more precise, but it
  forces the operator to choose again in every project. The spec rules it out.

## D2: A hide list, not an allow list
- **Decision:** persist the hidden names.
- **Why:** a new status or project shows by default, so work can never be
  hidden silently (FR-4). The request is phrased as hiding ("I don't need to
  see In Review").
- **Rejected:** an allow list. A newly created status would hide its
  workspaces until someone opted it in.

## D3: Data source for issue → status
- **Decision:** per-project `PROJECT_ISSUES_SHAPE` +
  `PROJECT_PROJECT_STATUSES_SHAPE` through `createShapeCollection`, gated on
  the dialog being open or a filter being set.
- **Why:** these are the same cached sources `LinkedIssueProvider` and
  `ProjectProvider` use, so no new backend endpoint is needed. The gate keeps
  the default sidebar free of new subscriptions.
- **Rejected:** (a) `SINGLE_ISSUE_SHAPE` per workspace, which opens N
  subscriptions (one per workspace rather than one per project); (b) a new
  backend endpoint that joins workspaces to statuses, which is new plumbing,
  against III; (c) `useShape` in a loop, which breaks the rules of hooks.

## D4: Fail open
- **Decision:** hide only when the status is known and hidden.
- **Why:** constitution XXXIV, and missing enrichment is not evidence
  (`docs/knowledge-base/workspace-summary-ordering.md`). Otherwise a slow sync
  would hide work unpredictably.

## D5: The "No issue" sentinel
- **Decision:** `__no_issue__`, mirroring the existing `__no_project__`
  sentinel. It matches workspaces whose remote row has `issue_id = null`, or
  that have no remote row at all. Both mean "not linked to an issue", and that
  fact does not depend on loading.
- **Note:** a local workspace whose remote row hasn't synced yet also looks
  unlinked. That is the same trade-off the existing "No project" option
  accepts.

## D6: UI primitive
- **Decision:** reuse `MultiSelectDropdown` (`@vibe/ui`) with `menuLabel`.
  The selected items are the hidden statuses, and the count badge shows how
  many are hidden.
- **Rejected:** a new checklist component, which would duplicate
  presentation (IV).
