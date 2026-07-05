# Spec: Collapse repeated `thinking_tokens` system log lines into a ticked single entry

Task: `4095-thinking-tokens`

## Problem

While a Claude Code session is running, the CLI emits `{"type":"system","subtype":"thinking_tokens",...}`
progress events repeatedly during long thinking blocks. Each event is normalized into its own
conversation entry with content `System: thinking_tokens` (see the unknown-subtype catch-all in
`ClaudeLogProcessor::normalize_entries`, `crates/executors/src/executors/claude.rs`).

On mobile especially, a single thinking block can produce 10+ identical consecutive log lines,
consuming most of the vertical scroll space (see task screenshots). The signal is useful
(progress is visible), but the repetition is not.

## Goal

When the same system message is emitted repeatedly with **no other conversation entry in
between** (an "uninterrupted run"), do not append a new entry. Instead, update the existing
entry in place, adding one tick mark (`✓`) per subsequent occurrence:

```
System: thinking_tokens          <- first occurrence
System: thinking_tokens ✓        <- after second occurrence (same entry, replaced)
System: thinking_tokens ✓✓✓      <- after fourth occurrence
```

Any interruption — an assistant message, a thinking entry, a tool call, a different system
subtype, stderr output, or any other entry allocated from the shared index provider — ends the
run; the next occurrence starts a fresh entry with no ticks.

## Scope

- **In scope:** the two catch-all branches of the `ClaudeJson::System` match in
  `ClaudeLogProcessor::normalize_entries`:
  - `Some(subtype)` → `System: {subtype}` (this is where `thinking_tokens` lands), and
  - `None` → `System message`.
  Both produce fixed, repeat-prone content and share the collapse logic.
- **Out of scope:** other executors (Codex, Gemini, …), the `status` subtype (variable
  content supplied by the CLI), frontend changes, and shared-type changes. The fix is entirely
  in backend normalization, so every consumer (local web, remote web, mobile) benefits, and no
  TS types are regenerated.

## Design

### State

Add to `ClaudeLogProcessor`:

```rust
/// Tracks the last catch-all system message so uninterrupted repeats
/// collapse into one ticked entry instead of new lines.
repeated_system_message: Option<RepeatedSystemMessage>,
```

```rust
struct RepeatedSystemMessage {
    entry_index: usize, // conversation index of the collapsed entry
    content: String,    // base content without ticks, e.g. "System: thinking_tokens"
    count: usize,       // occurrences so far (1 = no ticks yet)
}
```

### Algorithm

On each catch-all system event with base content `c` and raw-JSON metadata `m`:

1. If `repeated_system_message` is `Some(r)` with `r.content == c` **and**
   `entry_index_provider.current() == r.entry_index + 1` (i.e. the collapsed entry is still the
   most recently allocated index — nothing else was emitted since, including entries from
   other patch sources sharing the provider, e.g. stderr):
   - increment `r.count`,
   - emit `ConversationPatch::replace(r.entry_index, entry)` where the entry content is
     `format!("{c} {}", "✓".repeat(r.count - 1))` and metadata is `m` (latest event wins, so
     the newest raw payload stays inspectable).
2. Otherwise:
   - allocate a new index via `next()`, emit an ordinary add patch with content `c` and
     metadata `m`, and set `repeated_system_message = Some({index, c, count: 1})`.

The `current() == entry_index + 1` guard is the interruption detector: `EntryIndexProvider` is
shared across all patch producers for the process, so any interleaved entry advances it and
naturally breaks the run. No other bookkeeping (resetting the tracker in every other branch) is
required, which keeps the change local to the catch-all branches.

Note: in-place `replace` patches of *earlier* streaming entries (e.g. a delta updating an
already-allocated assistant message) do not advance the provider and therefore do not break a
run. That is acceptable: the collapsed entry is still the last line in the conversation, so
ticking it remains visually correct.

### Display

No frontend change. `SystemMessage` entries render their `content` as plain text/markdown in
`ChatSystemMessage`; the appended `✓` characters display as-is. Tick count is unbounded — even
a long run stays one wrapping line instead of N lines, which satisfies the goal.

## Testing

Unit tests alongside the existing `ClaudeLogProcessor` tests in `claude.rs`:

1. **Collapse:** four consecutive `system/thinking_tokens` events produce one add patch at
   index `i` followed by three replace patches at index `i`, final content
   `System: thinking_tokens ✓✓✓`; the provider's next index is still `i + 1`.
2. **Interruption:** `thinking_tokens`, then an assistant text message, then `thinking_tokens`
   again → two distinct entries, neither ticked.
3. **Different subtype:** `thinking_tokens` followed by another unknown subtype → two distinct
   entries.
4. **Existing tests** for unknown-subtype normalization keep passing (single occurrence is
   byte-identical to today's output).

## Acceptance criteria

- A stream containing N consecutive `thinking_tokens` events yields exactly one conversation
  entry whose content ends with N−1 tick marks.
- Any interleaved entry starts a new, untick­ed entry.
- `cargo test -p executors` passes; `pnpm run backend:check` clean.
