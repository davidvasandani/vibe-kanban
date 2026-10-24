# Collapse matching tool-call messages

Task: vk/d068-collapse-matchin

## Problem
When an agent calls the same generic or MCP tool over and over in a row (for
example `mcp:sgsc-mcp:solink` eight times in a row), the conversation shows one
identical row per call, each with its own timestamp. On mobile this fills the
whole screen with repeated rows and hides the useful content around them.

The conversation view already collapses runs of file reads, searches, web
fetches, categorized shell commands and same-file edits
(`packages/web-core/src/shared/lib/aggregateEntries.ts`). Generic `tool`
actions, which include every MCP tool, are never collapsed.

## Outcome
Two or more back-to-back `tool_use` entries whose `action_type.action` is
`tool` and whose `tool_name` matches are shown as one collapsible row:

`<tool icon> mcp:sgsc-mcp:solink · 8 calls`

- The row uses the same accordion component as other groups
  (`ChatAggregatedToolEntries`). Its status dot shows the worst status in the
  group, and its expanded state persists under the group key.
- When expanded, each call shows a one-line preview of its arguments
  (`key: value, …`, truncated). If there are no arguments, the row shows the
  tool name. Clicking a call opens its output in the logs panel, the same way a
  single tool row does.
- The group has one timestamp, chosen by the same rule as existing groups
  (the latest valid entry time).

## Rules
- Group only consecutive entries. Any other entry (assistant message, a
  different tool name, a different action, a user message) ends the run.
- A run of one stays a normal single row.
- Entries in `pending_approval` are never grouped. They keep the inline
  approval card, and they end any open run.
- Other aggregation behaviour (reads/searches/fetch/commands/diffs/thinking)
  is unchanged.
- The group key is `agg:<first entry patchKey>`, matching the existing
  `AGGREGATED_GROUP` convention, so it stays stable while entries stream in.

## Out of scope
- Grouping across different tools, or across tools that differ only in their
  arguments, beyond the shared `tool_name`.
- Backend/normalization changes. This is a frontend-only change.

## Acceptance
- Unit tests in `aggregateEntries.test.ts` cover: grouping same-name runs,
  splitting on a different name, single-entry passthrough, pending-approval
  exclusion, and interaction with existing aggregation types.
- `pnpm run check`, `pnpm run lint`, and the web-core Vitest suite pass.
