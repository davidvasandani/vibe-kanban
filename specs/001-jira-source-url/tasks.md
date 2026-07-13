# Tasks: Source Jira URL in the VK issue detail panel

**Plan**: `specs/001-jira-source-url/plan.md`
**Task**: `vk/a793-vk-jira-bi-direc`

`[P]` = parallelizable within its layer (no dependency on a sibling `[P]`).

## Layer 1 — Presentational component (packages/ui)
- [ ] T1. In `KanbanIssuePanel.tsx`: import `JiraBadge`; add optional prop
      `jiraLink?: { issueKey: string; url: string; active: boolean } | null` to
      `KanbanIssuePanelProps` and destructure it in the component signature.
- [ ] T2. (depends on T1) Render the badge in the header left id-group, after
      the copy-link button, gated `!isCreateMode && jiraLink`.

## Layer 2 — Container wiring (packages/web-core)  [depends on Layer 1 prop]
- [ ] T3. In `KanbanIssuePanelContainer.tsx`: destructure `getJiraLinkForIssue`
      from `useProjectContext()`; build a memoized `jiraLink` object from
      `getJiraLinkForIssue(selectedIssue.id)` and pass it to `<KanbanIssuePanel>`.

## Layer 3 — Tests & verification  [depends on Layers 1–2]
- [ ] T4. [P] Add a Vitest case to `KanbanIssuePanel.test.tsx`: badge present
      when `jiraLink` supplied (edit mode), absent when omitted / create mode.
- [ ] T5. [P] Run `pnpm run check` and `pnpm run lint`; confirm existing Rust
      `crates/remote/src/jira/mapping.rs` tests (Req 2, Done ⇄ Jira) stay green.
- [ ] T6. (depends on T4/T5) Run `pnpm run format`.

## Notes
- No backend/schema/type-generation work — the URL is already on the shape.
- Req 2 (Done ⇄ Jira) needs no code; it is asserted as still-green, not rebuilt.
