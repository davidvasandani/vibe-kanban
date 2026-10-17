# Duplicate issue workspace advisories

Issue identity comes from the remote workspace's `project_id` and `issue_id`, not its name. Names are usually copied from the issue title and cannot distinguish siblings from unrelated similarly named work. Active means `archived === false`, including idle and completed agent runs.

Project workspace shapes include all linked owners, while local sidebar records provide branch and activity enrichment. Join local records through `local_workspace_id`; join project PRs through the remote workspace `id`. These IDs are different. Never broadcast an issue-level PR fact to all sibling workspaces, and never use a PR target branch as the workspace branch. File-change counts are evidence of changes, not proof of unmerged commits.

Creation can render outside a ProjectProvider, so its advisory subscribes directly to the existing cached project workspace and PR shapes for the linked project. Missing enrichment does not hide the remote record or block creation. Dismissal is scoped to project, issue and sorted sibling IDs; PR/activity hydration does not reopen it, while a newly discovered sibling does. The issue section's plural active count lives in `CollapsibleSectionHeader.headerExtra` so it survives collapse.

## Host-safe navigation

Ownership does **not** prove a workspace is available on the currently selected host. Remote-host app navigation uses the current host when opening a workspace ID. The advisory offers Open only for an owned sibling with a matching current-host workspace record. Cross-host and other-owner siblings remain visible with unavailable metadata rather than linking to the wrong backend. Do not weaken this to an ownership-only check.

## Validation seams

`packages/remote-web/src/test/IssueWorkspaceWarning.test.tsx` covers identity, archives, per-workspace enrichment, other-owner and cross-host behavior, dismissal/new siblings, navigation and collapsed counts. Initialize the real shared i18n instance for readable rendered-label assertions: remote-web does not directly resolve the UI package's react-i18next dependency for a bare module mock. Translation keys must exist in every locale; English fallback alone does not satisfy `scripts/check-i18n.sh`.

## Contributed by

- `vk/fe2d-warn-when-starti` — duplicate-start advisory, passive active count, and cross-host navigation regression.
