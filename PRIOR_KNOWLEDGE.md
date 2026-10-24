# Prior knowledge — vk/d068-collapse-matchin

Searched `vibe-kanban/wiki` (read-only) for conversation rendering, aggregation,
collapsing, tool summaries, and the conversation timeline. The knowledge base
is not empty, but no page covers conversation-entry aggregation. The relevant
pages are:

- **issue-comment-delivery.md › Frontend › Conversation rows.** The thinking
  collapse in `aggregateEntries.ts` depends on turn boundaries
  (`user_message`). Do not insert synthetic user messages. New grouping must
  respect those boundaries, which the consecutive-only rule does.
  Context values that change every streamed frame cause every entry to
  re-render, so group keys must stay stable while entries stream in
  (`agg:<first patchKey>`).
- **frontend-linting.md.** ESLint directive comments are banned. Tests are
  type-linted through `tsconfig.eslint.json`. Run `pnpm run lint`, which also
  runs the unused-i18n-key check, so any new i18n key must be used.
- **markdown-tables.md.** Not directly relevant. It is the main conversation
  markdown renderer.

Code facts gathered for this task (not yet in the wiki):
- `aggregateConsecutiveEntries` (`packages/web-core/src/shared/lib/aggregateEntries.ts`)
  groups runs of file_read/search/web_fetch/command_run_{read,search,edit,fetch}
  into `AGGREGATED_GROUP`, same-file edits into `AGGREGATED_DIFF_GROUP`, and
  thinking in previous turns into `AGGREGATED_THINKING_GROUP`.
- MCP tools are normalized as `action: 'tool'` with
  `tool_name = "mcp:<server>:<tool>"` (claude.rs, codex, cursor), and they
  render one `ChatToolSummary` row per call. That is why the screenshot shows
  repeated rows.
- `ChatAggregatedToolEntries` (packages/ui) renders the accordion. It takes
  `label`, `unit` (auto-pluralized), and `icon`.
