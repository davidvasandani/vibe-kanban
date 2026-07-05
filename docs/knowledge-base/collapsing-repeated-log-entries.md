# Collapsing repeated log entries into ticked lines

Tags: `4095-thinking-tokens`

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
   `format!("{content} {}", "✓".repeat(count - 1))` — one tick per repeat, latest raw JSON
   kept as metadata.
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
- Tick string is unbounded by design: one wrapping line always beats N lines, and runs are
  bounded in practice by interruptions.

Implemented in `ClaudeLogProcessor::push_collapsible_system_message`
(`crates/executors/src/executors/claude.rs`), applied to both catch-all system-message
branches (`Some(subtype)` → `System: {subtype}`, `None` → `System message`).
