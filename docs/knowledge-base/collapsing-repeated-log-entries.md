# Collapsing repeated log entries into ticked lines

Tags: `4095-thinking-tokens`, `6aac-grok-same-comman`

## Problem

Agent CLIs can emit the same informational event many times in a row (Claude Code's
`thinking_tokens` system event fires repeatedly during one thinking block). Rendering each
occurrence as its own conversation entry floods the view — on mobile a single thinking block
consumed 10+ lines of `System: thinking_tokens`.

## Pattern

Collapse **uninterrupted** repeats server-side, in the log normalizer, so every consumer
(local web, remote web, mobile) benefits with zero frontend changes:

1. Keep `Option<RepeatedSystemMessage { entry_index, content, count }>` on the processor.
2. On a repeat-prone event, if the tracked `content` matches **and**
   `provider.current() == entry_index + 1` (the tracked entry is still the last allocated —
   see [claude-log-normalization](claude-log-normalization.md)), increment `count` and emit
   `ConversationPatch::replace(entry_index, ...)` with content
   one tick per repeat for short runs, then a compact counted marker (for
   example, `✓ ×42`) so marker allocation stays bounded by the number of digits
   in the count. Keep the latest raw JSON as metadata.
3. Otherwise allocate a fresh index, emit a plain add, and re-arm the tracker with
   `count: 1`.

Result: `System: thinking_tokens ✓✓✓` instead of four lines. Any interleaved entry (tool
call, assistant text, stderr line, different subtype) naturally breaks the run because it
advances the shared index provider — no per-branch reset code needed.

## Constraints learned

- **Invalidate the tracker wherever the index provider is reset** (AmpResume history wipe),
  or the stored index will point at a reallocated entry and the next repeat will overwrite
  it via `replace`.
- First occurrence must stay byte-identical to the old output so existing tests and
  downstream consumers are unaffected; ticks only appear from the second occurrence on.
- Never render an unbounded tick string. Historical replay and uninterrupted
  tool loops can make a run much longer than expected, and repeatedly building
  progressively larger replacement patches can exhaust the server's memory.

Implemented in `ClaudeLogProcessor::push_collapsible_system_message`
(`crates/executors/src/executors/claude.rs`), applied to both catch-all system-message
branches (`Some(subtype)` → `System: {subtype}`, `None` → `System message`).

## Collapsing repeated command-tool calls

Task `6aac-grok-same-comman` applied the same display pattern to consecutive
completed Claude `Bash` tool calls that invoke the exact same Grok command. This
case needs more state than repeated informational events:

- The screenshot's `grok --cwd ...` rows are outer Claude command-tool entries,
  not events inside Grok's ACP normalizer. Diagnose which normalizer owns the
  visible row before changing a vendor executor.
- Tool-call IDs remain unique even when several calls share one visible entry
  index. Keep every ID in the tool map so results can still be correlated.
- Only the latest compacted tool-call ID may refine the shared entry. A late
  result from an older ID must not overwrite the newer command state.
- Require the prior invocation to complete successfully before reusing its
  entry. A failure remains visibly failed and cannot become a success tick.
- Streaming re-emission of the same tool-call ID is an update, not a repeat; it
  must retain the current tick count without incrementing it.
- Scope command compaction narrowly (Grok executable plus exact normalized
  command text here). Generic identical-command suppression can hide deliberate
  repeated operations.

The shared-index invariant still applies: a different allocated entry ends the
run, and any index reset must clear both informational-event and command-call
trackers.
