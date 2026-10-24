# Conversation entry aggregation

How the chat view collapses runs of entries into accordion rows, and the
rules to follow when adding a new kind of group.

## Where it happens

`aggregateConsecutiveEntries` (`packages/web-core/src/shared/lib/aggregateEntries.ts`)
turns the flat entry list into `DisplayEntry[]`:

- `AGGREGATED_THINKING_GROUP` holds thinking entries in every turn except the
  last. Turn boundaries are `user_message` entries, so never insert synthetic
  user messages (see [issue-comment-delivery](issue-comment-delivery.md)).
- `AGGREGATED_DIFF_GROUP` holds consecutive `file_edit` entries for the same
  path.
- `AGGREGATED_GROUP` holds consecutive entries that share a
  `ToolAggregationType`: file_read, search, web_fetch,
  command_run_{read,search,edit,fetch}, and `tool`. `tool` also requires the
  same `tool_name`, which is stored as `group.toolName`.

`DisplayConversationEntry` → `AggregatedGroupEntry` maps each type to a
label, icon and unit for `ChatAggregatedToolEntries` (`packages/ui`).

## Rules

- **Prefer a new `ToolAggregationType` over a new group type.** The timeline
  (`deriveConversationTimeline.ts`), row model (`aggregated_tool`) and
  timestamps (`ConversationTimestamp.tsx`, which uses the latest valid entry
  time) all handle `AGGREGATED_GROUP` generically. A new group type has to be
  wired through every one of them.
- **Only group consecutive entries. A run of one is never wrapped.** The
  group key is `agg:<first patchKey>`, so persisted expansion
  (`tool:${patchKey}`) stays stable while a run grows during streaming.
- **Never group `pending_approval` entries.** `renderToolUseEntry` shows
  generic pending tools as an approval card, which would be hidden inside a
  collapsed group.
- **MCP tools** are normalized as `action: 'tool'` with
  `tool_name = "mcp:<server>:<tool>"` (Claude, Codex, Cursor). Single rows show
  only the tool name, so repeated calls look identical. Grouped rows show
  `formatToolArgsPreview(arguments)` per call so that the calls can be told
  apart, and they open `getToolOutput` (the `result.value`) in the logs panel.

## Contributed by

- `vk/d068-collapse-matchin`
