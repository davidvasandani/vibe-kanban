# Warn before starting duplicate issue workspaces

Task: vk/fe2d-warn-when-starti

## Outcome
When preparing a workspace linked to an issue, show a dismissible advisory if that exact issue already has non-archived workspaces. Identify each by name, branch and known status, and offer navigation to an existing workspace. Creating another workspace remains allowed without an acknowledgement gate. Display an active-workspace count on the issue when more than one exists.

## Contract
- Match the persisted issue UUID, never titles or fuzzy similarity.
- Count all linked non-archived workspaces; archived siblings do not trigger warnings.
- Reuse existing shared project/workspace data and navigation, including remote-only records.
- Highlight workspace-scoped open PR or unmerged-change evidence where available. Unknown evidence must remain unknown, not be inferred from another sibling.
- Dismissal is local to the current issue and observed sibling set; newly appearing siblings restore the advisory.
- Loading or unavailable enrichment must not block intentional starts.
- No backend rejection, hard uniqueness rule, service deployment, title deduplication or pre-merge overlap detection.

## Validation
Cover zero/one/multiple active siblings, archived records, identical titles on different issues, partial metadata, sibling-specific PR evidence, dismiss/reappear behavior, navigation and continued creation. Run relevant frontend checks, repository formatting and independent Codex review before knowledge capture and PR merge.
