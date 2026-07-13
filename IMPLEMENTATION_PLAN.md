# Implementation Plan: VK ↔ Jira source URL in the issue panel (task vk/a793-vk-jira-bi-direc)

Step-by-step build order. The authoritative dependency-ordered task list is
`specs/001-jira-source-url/tasks.md`; this is the repo-root summary.

## Pre-work (done during exploration)
- Confirmed the bidirectional reconciler + status sync already shipped (PR #85)
  and that **Req 2 (Done ⇄ Jira) needs no code** — verified in
  `crates/remote/src/jira/mapping.rs` (VK "Done" ⇄ Jira `done`-category).
- Confirmed the Jira URL is already on the client (`jira_browse_url` on the
  `jira_issue_links` shape; `getJiraLinkForIssue` in `ProjectProvider`) and is
  rendered on the card but **not** in the detail panel → the only gap.

## Step 1 — Panel component (packages/ui) [additive]
1. Import `JiraBadge` in `KanbanIssuePanel.tsx`.
2. Add optional prop `jiraLink?: { issueKey: string; url: string; active:
   boolean } | null` to `KanbanIssuePanelProps`; destructure it.
3. Render `<JiraBadge …/>` in the header left id-group after the copy-link
   button, gated `!isCreateMode && jiraLink`.

## Step 2 — Container wiring (packages/web-core) [depends on Step 1's prop]
4. Destructure `getJiraLinkForIssue` from the existing `useProjectContext()`.
5. Add a memoized `jiraLink` computed from `getJiraLinkForIssue(selectedIssue.id)`
   → `{ issueKey: jira_issue_key, url: jira_browse_url, active: link_state ===
   'active' }`, else `undefined`.
6. Pass `jiraLink={mode === 'edit' ? jiraLink : undefined}` to `<KanbanIssuePanel>`.

## Step 3 — Test (packages/remote-web) [depends on Steps 1–2]
7. Extend `renderPanel` in `KanbanIssuePanel.test.tsx` with a `jiraLink`
   override; add 3 cases: linked→badge present (href + target=_blank),
   unlinked→absent, create-mode→absent.

## Step 4 — Verify
8. `NODE_ENV=test` vitest on the panel test (5/5 green).
9. `ui:check`, `web-core:check`, `remote-web:check`, `local-web:check` (tsc).
10. `ui:lint`, `local-web:lint` (eslint). Prettier clean.
11. (Req 2 regression guard) `crates/remote/src/jira/mapping.rs` tests untouched.

## Notes / decisions
- No render prop — a leaf badge is passed as data, mirroring the card's contract
  exactly (single source of truth, FR-5).
- Header placement (not a new section) avoids the panel's border-convention
  pitfalls documented in `wiki/kanban-issue-panel-sections.md`.
- Blast radius: `KanbanIssuePanel` is shared local-web + remote-web (intended).
