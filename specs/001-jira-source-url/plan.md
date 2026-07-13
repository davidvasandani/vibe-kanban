# Technical Plan: Source Jira URL in the VK issue detail panel

**Spec**: `specs/001-jira-source-url/spec.md`
**Task**: `vk/a793-vk-jira-bi-direc`

## Approach
Reuse the exact contract the kanban card already uses. `KanbanCardContent`
takes `jiraLink?: { issueKey: string; url: string; active: boolean } | null` and
renders the shared `JiraBadge`. Mirror that one-to-one on `KanbanIssuePanel` and
have the container feed it from the same `getJiraLinkForIssue` lookup. No
backend, no new data source, no new component.

## Research notes
- The Jira URL is already streamed to the client on the link row
  (`jira_browse_url`) via `PROJECT_JIRA_LINKS_SHAPE`; `getJiraLinkForIssue` is
  exposed by `ProjectProvider` and consumed via `useProjectContext()`.
  `KanbanIssuePanelContainer` already calls `useProjectContext()` and already has
  `selectedIssue`. So the data is in hand — this is purely a presentational
  wiring change. No new dependency (constitution constraint satisfied).
- `active` is derived the same way the card derives it:
  `link.link_state === 'active'` (see `KanbanContainer` ~line 1190).

## Data model / contracts
No schema or API change. One new **optional presentational prop**:

```ts
// KanbanIssuePanelProps (packages/ui/src/components/KanbanIssuePanel.tsx)
jiraLink?: { issueKey: string; url: string; active: boolean } | null;
```

Identical to `KanbanCardContent`'s prop → one shared shape, one shared
`JiraBadge` renderer (FR-5).

## Changes
1. **`packages/ui/src/components/KanbanIssuePanel.tsx`**
   - Import `JiraBadge`.
   - Add `jiraLink?: { issueKey: string; url: string; active: boolean } | null`
     to `KanbanIssuePanelProps`; destructure it.
   - In the header's left id-group, after the copy-link button (~line 288),
     render `{!isCreateMode && jiraLink && <JiraBadge issueKey={jiraLink.issueKey}
     url={jiraLink.url} active={jiraLink.active} />}`.
2. **`packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`**
   - Pull `getJiraLinkForIssue` from the existing `useProjectContext()`
     destructure.
   - Compute `const jiraLink = selectedIssue ? getJiraLinkForIssue(selectedIssue.id) : undefined;`
     and pass `jiraLink={ link ? { issueKey: link.jira_issue_key, url:
     link.jira_browse_url, active: link.link_state === 'active' } : undefined }`
     to `<KanbanIssuePanel>` (memoize to avoid a new object each render).
3. **`packages/remote-web/src/test/KanbanIssuePanel.test.tsx`**
   - Add a test: with `jiraLink` set + `mode: 'edit'`, the badge (an anchor
     labeled with the Jira key) renders; with `jiraLink` omitted it does not.

## Testing
- `pnpm run check` (frontend + Rust) and `pnpm run lint`.
- Vitest panel test above.
- The unchanged Rust `mapping.rs` tests continue to prove Req 2 (Done ⇄ Jira).
- `pnpm run format` before completion.

## Risks / rollback
- Blast radius: `KanbanIssuePanel` is shared by local-web and remote-web — the
  header badge appears in both. Intended.
- Purely additive + optional prop → trivially reversible; unlinked issues and
  create mode are unaffected (FR-4).
