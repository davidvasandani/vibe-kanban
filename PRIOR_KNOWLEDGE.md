# Prior Knowledge — recalled for `vk/a793-vk-jira-bi-direc`

Searched the project knowledge base (`wiki/` — INDEX + topic pages) for pages
relevant to this task: surfacing the source Jira ticket URL in the VK issue
detail panel, and confirming VK-Done ⇄ Jira status sync. **Two pages are
directly on-topic** — this task builds heavily on recorded knowledge.

## Directly relevant pages (reused)

- **[external-connector-sync.md]** (from `vk/d2aa-sync-vk-and-jira`) — the whole
  Jira connector design. Key facts this task relied on:
  - The connector lives in the **remote** stack (`crates/remote`), not the local
    SQLite model; issues are `Issue` rows streamed via Electric shapes. The Jira
    link is a `jira_issue_links` row (`jira_issue_key`, `jira_browse_url`,
    `link_state`) streamed via `PROJECT_JIRA_LINKS_SHAPE` — **the URL is already
    on the client**, so surfacing it in the panel is pure presentational wiring
    (no backend/schema/type work).
  - Echo-free per-field 3-way merge already syncs status both ways → **Req 2
    (Done ⇄ Jira) is already done and tested** (`mapping.rs`); do not rebuild it.

- **[kanban-issue-panel-sections.md]** (from `vk/b37f-move-issue-works`,
  `vk/77eb-vk-pipeline`) — the panel this task edits. Key facts:
  - `KanbanIssuePanel.tsx` (in `packages/ui`) **owns its own layout**; the
    container (`KanbanIssuePanelContainer.tsx` in `web-core`) only supplies data.
    The panel is shared by local-web **and** remote-web → both frontends are the
    blast radius. (I added a leaf data prop + badge, not a new bordered section,
    so no section reorder / border-convention concern.)
  - **Testing recipe** (used verbatim for the new test): `@vibe/ui` component
    tests live in `packages/remote-web/src/test/*.test.tsx` (jsdom +
    testing-library). **`NODE_ENV` gotcha**: the dev env exports
    `NODE_ENV=production`, which breaks testing-library; run with `NODE_ENV=test`
    (I ran `NODE_ENV=test npx vitest run ...`). Without an i18n provider `t()`
    returns raw keys → assert on aria-labels / roles / testids, not translated
    strings (my test asserts on the badge's link role + Jira key, not `t()`).

## Constraints carried into design (constitution + wiki)

- Reuse over new plumbing: the card already renders `JiraBadge` from a
  `{ issueKey, url, active }` prop fed by `getJiraLinkForIssue`. I mirrored that
  exact shape and lookup on the panel → single source of truth, card and panel
  can't diverge.
- Additive + optional prop → small, reversible; unlinked issues and create mode
  unaffected.

## Enrichment note (stage 12)
`external-connector-sync.md` is about the reconciler/backend; it does not cover
*where the connector link surfaces in the UI*. That's a small, reusable gap —
stage 12 adds a short "Surfacing the link in the UI" note to
`kanban-issue-panel-sections.md` (card + panel share one `JiraBadge`/prop/lookup).
