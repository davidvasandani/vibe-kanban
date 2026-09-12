# Claude executor log normalization

Tags: `4095-thinking-tokens`, `6aac-grok-same-comman`

## Pipeline shape

`ClaudeLogProcessor` (`crates/executors/src/executors/claude.rs`) parses Claude Code's
stream-JSON stdout line by line and emits `json_patch::Patch` updates over a virtual
`/entries/{index}` array. Two patch kinds matter:

- `ConversationPatch::add_normalized_entry(idx, entry)` — append a new conversation entry.
- `ConversationPatch::replace(idx, entry)` — update an existing entry in place. Used whenever
  a later event refines an earlier one (tool results replacing tool calls via `tool_map`,
  `task_progress`/`task_notification` updating Task entries, repeated system messages gaining
  ticks).

Entry indices come from `EntryIndexProvider`
(`crates/executors/src/logs/utils/entry_index.rs`): a shared `Arc<AtomicUsize>` cloned into
**every** patch producer for a process — stdout normalizer, stderr normalizer, etc. — so
indices are globally ordered across streams.

## Useful idioms

- **"Was I the last entry?"** — if you stored `idx = provider.next()`, then
  `provider.current() == idx + 1` means nothing else has been allocated since, across all
  producers. This is how uninterrupted runs of repeated system messages are detected without
  any cross-branch bookkeeping. Caveat: in-place `replace` patches of earlier entries do not
  advance the provider, so they do not count as interruptions.
- **Unknown system subtypes** fall through to a catch-all that renders
  `System: {subtype}` with the raw JSON as metadata. Claude Code emits some subtypes very
  frequently (e.g. `thinking_tokens` during long thinking blocks), so anything in the
  catch-all path must tolerate high-frequency repeats — see
  [collapsing-repeated-log-entries](collapsing-repeated-log-entries.md).
- **Compacted tool calls still need unique lifecycle identity.** Multiple
  completed identical Grok `Bash` calls may share one visible entry index, but
  `tool_map` must retain each tool-call ID. The latest call owns replacements of
  that shared row; stale results are ignored. Repeated stream updates for the
  same ID preserve ticks without incrementing the repeat count.

## Gotcha: AmpResume resets entry indices

Under `HistoryStrategy::AmpResume`, the first real user text message wipes replayed history:
it emits N remove patches, calls `entry_index_provider.reset()`, and clears `tool_map`.
**Any processor state that stores entry indices must be invalidated at that reset point**
(`tool_map` and `repeated_system_message` both are). A stale stored index otherwise causes a
`replace` that overwrites whatever entry got reallocated at that index after the reset —
this exact bug was caught by Codex review in task `4095-thinking-tokens`.
The same reset also clears `repeated_grok_command`.

## Testing pattern

Unit tests drive `normalize_entries(&ClaudeJson, worktree, &EntryIndexProvider)` directly
with `EntryIndexProvider::test_new()` and inspect patches via
`extract_normalized_entry_from_patch(&patch) -> Option<(usize, NormalizedEntry)>` (works for
both add and replace ops). Assert the op kind with
`matches!(patch.0.first(), Some(PatchOperation::Add(_) | PatchOperation::Replace(_)))`.
