# Implementation plan — collapse matching tool-call messages

Task: vk/d068-collapse-matchin · Spec: `SPEC.md`

1. **Types** (`packages/web-core/src/shared/hooks/useConversationHistory/types.ts`)
   - Add `'tool'` to `ToolAggregationType`.
   - Add an optional `toolName?: string` to `AggregatedPatchGroup`. Groups of
     type `tool` set it.
2. **Aggregation** (`packages/web-core/src/shared/lib/aggregateEntries.ts`)
   - `getAggregationType` returns `'tool'` for `action === 'tool'` unless
     the status is `pending_approval`.
   - Track the current run's tool name. A `tool` entry with a different
     `tool_name` flushes the run and starts a new one.
   - `flushToolGroup` writes `toolName` onto the group.
3. **Rendering** (`packages/web-core/src/features/workspace-chat/ui/DisplayConversationEntry.tsx`)
   - `AggregatedGroupEntry`: for `tool` entries, summary = argument preview
     (falling back to the tool name), and content = `getToolOutput(...)` so
     "view content" opens the real result.
   - `getDisplayProps`: `case 'tool'` → label = `group.toolName`, unit
     `call`, `ListMagnifyingGlassIcon`. This matches the single-row icon for
     generic tools in `ChatToolSummary`.
   - Add a small pure helper `formatToolArgsPreview(args)` in
     `aggregateEntries.ts` (exported, unit-tested). It takes the top-level
     `key: value` pairs, stringifies scalars and abbreviates objects/arrays,
     and truncates to ~120 characters.
4. **Tests** (`packages/web-core/src/shared/lib/aggregateEntries.test.ts`, new)
   - Same-name run → one group with `toolName` and the correct key.
   - A different name splits the run. A single entry passes through.
   - `pending_approval` is never grouped and breaks the run.
   - A non-tool entry between two calls splits them. Existing `file_read`
     grouping still works.
   - `formatToolArgsPreview` cases: null, empty object, scalars, nested
     values, truncation.
5. **Verify**: run `pnpm install --frozen-lockfile` if needed, then
   `pnpm --filter web-core test` (vitest), `pnpm run check`, `pnpm run lint`,
   and `pnpm run format`.
