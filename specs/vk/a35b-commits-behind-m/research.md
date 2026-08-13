# Research: Commits Behind in the Git Header

## Existing query seam

`packages/web-core/src/shared/hooks/useBranchStatus.ts` is the authoritative
frontend subscription for `RepoBranchStatus[]`. It polls every 15 seconds,
avoids background polling, and uses the key `['branchStatus', workspaceId]`.
The expanded `GitPanelContainer` already interprets `commits_behind` from this
response.

**Decision:** subscribe from a header-owned component using the same hook.
TanStack Query shares/deduplicates the request, and the header remains mounted
when `CollapsibleSectionHeader` unmounts the expanded body.

**Rejected:** lift the full Git panel's data mapping into `RightSidebar`. That
would broaden a focused feature, couple action-heavy body state to the sidebar,
and duplicate much of `GitPanelContainer`'s responsibility.

**Rejected:** report status upward from `GitPanelContainer`. The child is absent
when collapsed, precisely when the new indicator must remain visible.

## Presentation ownership

The existing `headerExtra` accepts feature-owned React content, and Server
Affinity establishes the local convention for a bounded, truncating metadata
span.

**Decision:** keep the Git-specific presentation in web-core and reuse the
shared collapsible primitive unchanged.

## Count semantics

The backend branch-status route calculates divergence against the repository's
configured target and returns nullable values. Targets may be remote-prefixed,
so re-running Git comparison in the browser is neither possible nor correct.

**Decision:** positive numeric `commits_behind` is the only displayable signal.
Null/loading is unavailable; zero is current; neither renders a warning.

## Multi-repository semantics

An aggregate sum loses the repository mapping and can mislead users about which
branch needs action.

**Decision:** preserve repository input order, join by `repo_id`, and name each
positive entry whenever the workspace has multiple repositories.

No new package, API, or external research is needed.
