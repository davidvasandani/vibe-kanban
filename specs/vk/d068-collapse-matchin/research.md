# Research — vk/d068-collapse-matchin

## Decision: extend `AGGREGATED_GROUP` rather than add a new group type
- Rationale: the timeline (`deriveConversationTimeline.ts`), the row model
  (`conversation-row-model.ts`, `aggregated_tool`) and timestamp handling
  (`ConversationTimestamp.tsx`) already understand `AGGREGATED_GROUP`. A new
  type would need changes in all of them.
- Alternative rejected: a separate `AGGREGATED_TOOL_CALL_GROUP`. It duplicates
  plumbing for no behavioural gain.

## Decision: group key = tool name only
See `clarifications.md`. Grouping on name and arguments would leave the
reported case uncollapsed.

## Decision: exclude `pending_approval`
`renderToolUseEntry` renders `GenericToolApprovalEntry` for pending generic
tools. Inside a group that card would disappear behind the accordion.

## Contract: `formatToolArgsPreview(args: JsonValue | null, max = 120): string`
- `null` or an empty object/array → `''`. The caller falls back to the tool name.
- Object → `key: value` pairs joined with `, `. Strings appear unquoted and on
  one line, as do numbers, booleans and `null`. Arrays render as `[n]` and
  objects as `{…}`.
- The result is truncated to `max` characters with a trailing `…`.
- A non-object `args` (string/number) is rendered as a single-line string.

## No new dependencies.
