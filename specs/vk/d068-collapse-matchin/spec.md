# Feature Specification: Collapse matching tool-call messages

**Feature dir**: `specs/vk/d068-collapse-matchin/`
**Status**: Draft

## Summary
Agents often call the same tool many times in a row, especially MCP tools such
as `mcp:sgsc-mcp:solink`. The conversation then shows one identical row per
call, each with its own timestamp. On a phone that pushes everything useful off
screen. The view already collapses runs of reads, searches, fetches,
categorized commands and same-file edits. This feature does the same for
repeated calls to one generic/MCP tool: show a single expandable row that names
the tool and the number of calls.

## User Stories
- As someone following an agent on mobile, I want a run of identical tool calls
  to take up one row, so that I can see the agent's messages without scrolling
  past dozens of duplicates.
- As someone debugging an agent, I want to expand that row and open any single
  call's output, so that collapsing hides no information.

## Functional Requirements
- FR-1: Two or more consecutive generic tool calls (MCP or other non-specialised
  tools) with the same tool name are shown as one group row.
- FR-2: The group row shows the tool name and the call count, e.g.
  `mcp:sgsc-mcp:solink · 8 calls`. Its status indicator shows the worst status
  among the calls.
- FR-3: Expanding the group lists every call, each with a short one-line
  preview of its arguments. Selecting a call opens its output, just as
  selecting an uncollapsed call does.
- FR-4: Only consecutive calls are grouped. Any other entry between them, or a
  call to a differently named tool, starts a new group.
- FR-5: A call that is waiting for the user's approval is never collapsed, and
  it ends the current run, so that approval prompts stay visible.
- FR-6: A single call is shown exactly as it is today.
- FR-7: The open/closed state of a group survives re-renders and new streamed
  entries, as other groups already do.
- FR-8: Existing grouping of reads, searches, fetches, commands, edits and
  thinking does not change.

## Out of Scope
- Grouping calls to different tools, or non-adjacent calls.
- Changing how agents' logs are normalized on the backend.

## Acceptance Criteria
- [ ] A conversation with 8 consecutive `mcp:sgsc-mcp:solink` calls renders
      one row reading `mcp:sgsc-mcp:solink · 8 calls`.
- [ ] Expanding that row shows 8 rows with argument previews. Clicking one opens
      that call's output.
- [ ] `A, A, B, A` (tool names) renders as group(A×2), B, A.
- [ ] A pending-approval call between two same-name calls shows its approval
      card, and the calls on either side are not merged across it.
- [ ] Unit tests cover the grouping rules and the argument preview. The
      frontend type check, lint and tests pass.

## Clarifications
- Group by tool name only. The screenshot shows rows that look identical
  because only the tool name is displayed, whatever the arguments. Grouping by
  name removes exactly the duplication the user sees. The argument previews in
  the expanded list keep the individual calls distinguishable.
- Use the same icon as an ungrouped generic tool row (list-magnifier). A group
  should look like the rows it replaces, and no dedicated MCP icon exists.

## Open Questions
- None remaining.
