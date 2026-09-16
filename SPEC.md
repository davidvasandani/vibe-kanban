# Archive linked workspaces when an issue is marked Done

Task: vk/88c5-marking-an-issue

## Outcome
Changing an issue to Done archives its linked workspaces, so they leave the active workspace list in the right drawer and remain accessible through existing archived workspace controls.

## Requirements
- Apply the behavior on a successful transition into the project's Done status, using the existing status model.
- Archive every linked, unarchived workspace; preserve already archived workspaces and unrelated issues/workspaces.
- Persist archive state through the existing workspace lifecycle and propagate updates to the drawer without requiring a page reload.
- Cover individual and bulk status mutation paths where supported. A failed status change must not archive workspaces.
- Reopening an issue does not automatically unarchive workspaces. Archiving must not delete workspaces or their history.
- Limit implementation to Vibe Kanban; no other service changes.

## Technical direction
Inspect issue status persistence, local/remote workspace links, and existing archive operations. Implement at the shared mutation boundary where possible, preserving authorization and existing lifecycle side effects. Reuse live workspace updates and add regression coverage for Done, other statuses, repeated updates, multiple links, and unrelated workspaces.

## Verification and delivery
Complete the requested ordered pipeline, focused tests and repository checks, independent Codex review, reusable project knowledge, and a merged pull request. Refine this specification after the required prior-knowledge recall and SpecKit analysis.

## Confirmed implementation boundary
Prior-knowledge recall and code tracing found remote single/bulk updates already
archive their remote workspaces. The missing boundary is LinkedIssueProvider in
the workspace drawer: unlike ProjectProvider, it does not reconcile archive state.
Subscribe there to the existing project workspace shape, filter by linked issue,
and reuse the existing reconciliation hook and local archive endpoint. No schema,
backend, or deployment changes are required. Task-specific SpecKit artifacts live
at `../homelab/specs/vk/88c5-marking-an-issue/` as specified by the workspace commands.
