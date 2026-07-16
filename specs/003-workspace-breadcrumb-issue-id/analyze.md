# Analysis: Workspace Breadcrumb Issue ID

**Analyzed**: 2026-07-15
**Inputs**: [`spec.md`](spec.md), [`plan.md`](plan.md),
[`research.md`](research.md), [`tasks.md`](tasks.md),
[`../../.specify/memory/constitution.md`](../../.specify/memory/constitution.md)

## Summary

The spec packet is mostly consistent and implementation-ready after the
documentation fixes recorded below. The requirements map to concrete tasks, the
plan keeps issue-label resolution in the container layer, and the proposed work
does not require backend, generated type, dependency, or presentational UI
changes.

Primary remaining risk: implementation must preserve the existing
`RemoteIssueLink` loading suppression while breadcrumbs are deferred, otherwise
the page could show a temporary non-breadcrumb issue affordance that conflicts
with the loading-state acceptance criteria.

## Documentation Fixes Applied

- Updated [`plan.md`](plan.md) to replace a stale constitution reference to
  "One MCP contract for all agents" with the current shared-component,
  don't-rebuild, and workspace-breadcrumb issue-identity principles.
- Updated [`plan.md`](plan.md) and [`tasks.md`](tasks.md) to explicitly preserve
  `RemoteIssueLink` loading suppression while linked issue breadcrumbs are
  deferred.
- Fixed a typo in [`research.md`](research.md): `FR-004a` now references
  `FR-004`.
- Promoted the responsive-shortening edge case into formal coverage by adding
  `FR-013`, `AC-010`, validation language, and resolved-issue test coverage in
  [`tasks.md`](tasks.md).

## Requirement-to-Task Coverage

| Requirement | Coverage | Tasks | Notes |
| --- | --- | --- | --- |
| FR-001: render issue crumb between project and workspace only after resolved/unavailable | Covered | T004, T005, T006, T007, T008, T010 | Helper models issue states and emits the middle crumb only for resolved/unavailable. |
| FR-002: use `simple_id` when issue record is available | Covered | T004, T005, T007, T008 | Research and plan reject raw ID fallback. |
| FR-003: defer linked trail while issue record is loading | Covered | T004, T007, T009 | T006 now also preserves fallback-slot loading suppression. |
| FR-004: never use UUIDs or non-issue-display fallback labels | Covered | T005, T007, T009, T010, T016 | Review task explicitly checks accidental UUID/fallback labels. |
| FR-005: resolved issue crumb opens linked issue in linked project | Covered | T005, T006, T008 | Navigation callback uses `goToProjectIssue(projectId, issueId)`. |
| FR-006: preserve workspaces without linked issues | Covered | T005, T011 | Existing `Project / Workspace` behavior remains only for unlinked workspaces. |
| FR-007: preserve project pages and other non-workspace surfaces | Covered | T006, T011, T016 | Plan keeps `isOnProjectPage` bypass. |
| FR-008: presentational navbar receives prepared items only | Covered | T003, T004, T006, T015 | Plan avoids `packages/ui` API or behavior changes. |
| FR-009: focused automated tests for resolved/loading/unavailable | Covered | T008, T009, T010, T012 | Verification task runs the focused suite. |
| FR-010: completed no-match renders unavailable crumb | Covered | T004, T005, T007, T010 | Requires loading complete before unavailable. |
| FR-011: unavailable crumb is distinct and non-navigable | Covered | T005, T010 | Tests assert exact label and no issue-opening action. |
| FR-012: temporary async absence is not unavailable/unlinked | Covered | T004, T007, T009, T016 | Review focuses on false unavailable states during startup. |
| FR-013: responsive/space-constrained rendering preserves distinct issue identity | Covered | T008, T015, T016 | Coverage is item-construction focused; no existing breadcrumb DOM tests were found for this surface. |

## Acceptance Criteria Coverage

| Acceptance Criterion | Coverage | Tasks |
| --- | --- | --- |
| AC-001: exactly one issue breadcrumb between project/workspace when resolved | Covered | T008 |
| AC-002: resolved label equals `simple_id` | Covered | T008 |
| AC-003: loading renders no partial `Project / Workspace` and no fallback label | Covered | T009, T006 |
| AC-004: unavailable renders exactly one unavailable crumb between project/workspace | Covered | T010 |
| AC-005: resolved issue crumb navigates with linked project and issue IDs | Covered | T008 |
| AC-006: workspace without linked issue renders no issue crumb | Covered | T011 |
| AC-007: project-page breadcrumb behavior unchanged | Covered | T011, T016 |
| AC-008: unavailable label exactly `Issue unavailable` and has no issue action | Covered | T010 |
| AC-009: no issue-linked loading/unavailable state renders `Project / Workspace` | Covered | T009, T010 |
| AC-010: long surrounding labels still keep a distinct `simple_id` issue crumb | Covered | T008 |

## Consistency Findings

- Resolved: [`plan.md`](plan.md) was out of sync with the current constitution.
  It referenced an older MCP principle and omitted the new workspace breadcrumb
  issue-identity principle. The constitution check has been updated.
- Resolved: [`plan.md`](plan.md) previously described `RemoteIssueLink` as a
  likely temporary affordance while breadcrumbs are deferred. Current
  `NavbarContainer` already suppresses that slot during loading, and the spec's
  loading acceptance criteria require no fallback issue label. The plan and
  task list now preserve the loading guard.
- Resolved: [`research.md`](research.md) referenced `FR-004a`, but the spec only
  defines `FR-004`. The reference has been corrected.
- No unresolved conflicts found between the spec, plan, research, and tasks.

## Gaps and Risks

- `RemoteIssueLink` is outside the pure breadcrumb helper. T006 must preserve
  the container-level loading guard, not just helper behavior, or AC-003 could
  pass at the helper level while the rendered navbar still shows a temporary
  issue affordance.
- Project-page behavior is primarily covered by bypass behavior in
  `NavbarContainer`. If the helper has no project-page concept, T011 should
  assert that the container does not call the workspace breadcrumb path for
  project destinations, or T016 should explicitly verify that the bypass remains
  intact.
- No existing breadcrumb/Navbar component tests were found under the frontend
  test files, so the constitution does not require adding a rendered-DOM test
  for an existing surface. If implementation touches `packages/ui` despite the
  plan, this analysis no longer holds and DOM coverage should be reconsidered.

## Constitution Findings

- **I. Clarity over cleverness**: Satisfied. The spec and plan define explicit
  loading/resolved/unavailable states and avoid implicit fallback behavior.
- **II. Test the contract**: Satisfied. Acceptance criteria exist before
  implementation, and tasks T008-T012 cover focused automated validation.
- **III. Small, reversible steps**: Satisfied. The plan reuses existing
  workspace, project, issue, and navigation data sources without new plumbing.
- **IV. Shared-component boundaries are law**: Satisfied. Issue label resolution
  remains in `web-core`; `packages/ui` remains presentational.
- **V. Remote mutations are transactional and txid-covered**: Not applicable.
  This feature has no remote mutation path.
- **VI. Don't rebuild what shipped**: Satisfied. The plan extends existing
  `NavbarContainer` behavior, `useShape`, `useAllOrganizationProjects`, and app
  navigation APIs.
- **VII. Workspace breadcrumbs preserve issue identity**: Satisfied after the
  documentation fix adding `FR-013`/`AC-010`. The feature requires `simple_id`
  for resolved issue breadcrumbs and forbids UUID or non-issue fallback labels.

## Recommendation

Proceed to implementation with the task list as updated. The highest-signal
review point is to ensure loading cannot fall through to either a completed
`Project / Workspace` breadcrumb trail or an out-of-band `RemoteIssueLink`
fallback.
