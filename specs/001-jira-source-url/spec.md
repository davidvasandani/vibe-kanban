# Feature Specification: Source Jira URL in the VK issue detail panel

**Feature dir**: `specs/001-jira-source-url/`
**Status**: Draft
**Task**: `vk/a793-vk-jira-bi-direc`

## Summary
Vibe Kanban already runs a bidirectional Jira sync: issues imported from Jira
carry a link row (`jira_issue_links`) holding the issue key and its
`jira_browse_url`, and status changes flow both ways (including VK "Done" ⇄ a
Jira `done`-category status). That link is surfaced today only as a small
`JiraBadge` on the kanban **card**. When a user opens an issue in the detail
panel, there is no visible pointer back to the source Jira ticket. This feature
surfaces the source Jira ticket link inside the issue detail panel so the origin
of a synced issue is discoverable from the place users read and edit it.

## User Stories
- As a team member reviewing a synced issue, I want to see and click through to
  its source Jira ticket from the issue detail panel, so I can check the
  original context without hunting for the card on the board.
- As a team member, I want the panel link to visibly indicate when the issue is
  no longer actively syncing (dormant / deleted in Jira), so I don't trust a
  stale pointer.

## Functional Requirements
- FR-1: When an issue opened in the detail panel has an associated Jira link,
  the panel MUST display the Jira issue key and a link to its
  `jira_browse_url`.
- FR-2: Activating the link MUST open the Jira ticket in a new browser tab
  without navigating away from or losing the VK panel state.
- FR-3: When the underlying link is not active (dormant or deleted-in-Jira), the
  panel indication MUST be visually de-emphasized / labeled as no longer
  syncing, consistent with how the card badge already conveys this.
- FR-4: When an issue has no Jira link, the panel MUST render exactly as it does
  today (no empty section, no placeholder).
- FR-5: The panel link MUST reflect the same link data the card uses (single
  source of truth); it MUST NOT introduce a second lookup path that could
  diverge from the card.
- FR-6: The existing VK ⇄ Jira status synchronization (including marking an
  issue Done on either side) MUST remain unchanged and continue to function.

## Out of Scope
- Changing the reconciler, status mapping, or any `crates/remote/src/jira/`
  backend behavior.
- Adding the Jira URL to the issue's stored `extension_metadata` or persisting a
  new field (the link row + shape already carry the URL).
- Editing/unlinking the Jira association from the panel.
- Surfacing the link anywhere other than the card (existing) and the detail
  panel (this feature).

## Acceptance Criteria
- [ ] Opening a Jira-linked issue shows a clickable element with the Jira key
      that points at the issue's `jira_browse_url`.
- [ ] Clicking it opens the ticket in a new tab; the panel stays open.
- [ ] A dormant / deleted-remote link renders de-emphasized (same signal as the
      card badge's `active=false`).
- [ ] An issue with no Jira link shows no Jira element and the panel layout is
      byte-for-byte the pre-change layout for that case.
- [ ] The rendered-DOM panel test asserts the Jira element's presence/position
      for a linked issue and its absence for an unlinked issue.
- [ ] Marking a VK issue Done still transitions the linked Jira issue (unchanged
      behavior; verified by the existing `mapping.rs` tests remaining green).

## Clarifications (resolved)
- **Placement**: The Jira badge renders in the panel **header**, in the left
  id-group immediately after the "copy link" button
  (`KanbanIssuePanel.tsx` ~line 288), edit-mode only. Rationale: it sits next to
  the issue's `displayId`, is always visible without scrolling, and reads as
  "this issue ← its Jira ticket". A Jira link only exists for a persisted synced
  issue, so create mode never shows it. This avoids a new bordered section
  (respects the panel's border convention) and needs no section reordering.
- **Data flow**: Add one optional data prop
  `jiraLink?: { issueKey: string; url: string; active: boolean }` to
  `KanbanIssuePanelProps`, mirroring the existing `KanbanCardContent` contract,
  and render it with the existing `JiraBadge` component. The container
  (`KanbanIssuePanelContainer`) computes it from
  `getJiraLinkForIssue(selectedIssue.id)` — the **same** lookup the card uses
  (exposed via `useProjectContext()`), satisfying FR-5 (single source of truth,
  no divergent second path). No render prop is needed because this is a leaf
  badge, not a container-owned section.

## Open Questions
- None remaining.
