# Technical Spec: VK ↔ Jira — source URL in the issue, Done ⇄ Jira status

> Task `vk/a793-vk-jira-bi-direc`. Full SpecKit artifacts live in
> `specs/001-jira-source-url/` (`spec.md`, `plan.md`, `tasks.md`, `analyze.md`)
> and the constitution in `.specify/memory/constitution.md`. This file is the
> repo-root technical summary.

## Context — what already shipped (PR #85)

Bidirectional Jira sync already exists in `crates/remote/src/jira/`. It imports
Jira issues into VK, stores the source link on `jira_issue_links`
(`jira_issue_key`, `jira_browse_url`, `link_state`) — streamed to the client via
`PROJECT_JIRA_LINKS_SHAPE` — and runs an echo-free per-field 3-way merge
(title/description/status) in **both** directions.

## The two requirements vs. reality

1. **Include the source Jira URL in the VK issue.** The URL was stored and shown
   as a `JiraBadge` on the kanban **card**, but *not* in the issue detail panel.
   → This is the one real gap; this task closes it.
2. **Marking a VK issue Done updates Jira (and vice versa).** Already
   implemented: `mapping.rs` maps VK's "Done" column ⇄ any Jira `done`-category
   status, seeded from observed statuses; the reconciler transitions the linked
   issue on either side. Unit-tested. → No code change; asserted still-green.

## Solution (Req 1 gap)

Frontend-only, additive, reusing the card's exact contract.

### 1. `packages/ui/src/components/KanbanIssuePanel.tsx`
Add optional prop `jiraLink?: { issueKey: string; url: string; active: boolean }
| null` (identical to `KanbanCardContent`), and render the shared `JiraBadge` in
the panel **header** — in the left id-group, right after the copy-link button,
gated `!isCreateMode && jiraLink`. Placement chosen to sit beside the issue's
`displayId`, always visible, no new bordered section (respects the panel's
border convention).

### 2. `packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`
Destructure the already-available `getJiraLinkForIssue` from
`useProjectContext()`; compute a memoized `jiraLink` from
`getJiraLinkForIssue(selectedIssue.id)` (`active = link_state === 'active'`), and
pass it edit-mode-only. Same lookup the card uses → card and panel can't diverge.

### 3. `packages/remote-web/src/test/KanbanIssuePanel.test.tsx`
Add rendered-DOM cases: badge present when linked (edit mode), absent when
unlinked, absent in create mode.

## Scope

No Rust, API, schema, migration, or generated-type changes — the URL is already
on the link shape. The reconciler and status mapping are untouched, so Req 2 is
preserved.

## Validation

- `KanbanIssuePanel.test.tsx`: 5/5 pass (2 existing + 3 new).
- `ui:check`, `web-core:check`, `remote-web:check`, `local-web:check` (tsc) pass.
- `ui:lint`, `local-web:lint` (eslint) pass; Prettier clean.
- Existing `crates/remote/src/jira/mapping.rs` tests unchanged (Req 2).

## Files

- `packages/ui/src/components/KanbanIssuePanel.tsx` (edited)
- `packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx` (edited)
- `packages/remote-web/src/test/KanbanIssuePanel.test.tsx` (edited)
