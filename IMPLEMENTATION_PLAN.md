# Implementation Plan: Collapse repeated `thinking_tokens` system log lines

Task: `4095-thinking-tokens` · Spec: `SPEC.md` · Prior knowledge: `../PRIOR_KNOWLEDGE.md`

All changes are in `crates/executors/src/executors/claude.rs`. No frontend, DB, or
shared-type changes.

## Step 1 — Add repeat-tracking state to `ClaudeLogProcessor`

- Define a private struct near `StreamingMessageState`:
  ```rust
  struct RepeatedSystemMessage {
      entry_index: usize,
      content: String,
      count: usize,
  }
  ```
- Add field `repeated_system_message: Option<RepeatedSystemMessage>` to
  `ClaudeLogProcessor` (around line 748) and initialize it to `None` in
  `new_with_strategy`.

## Step 2 — Add a collapse helper

Add a method on `ClaudeLogProcessor`:

```rust
fn push_collapsible_system_message(
    &mut self,
    content: String,
    metadata: Option<serde_json::Value>,
    entry_index_provider: &EntryIndexProvider,
) -> json_patch::Patch
```

Behavior:
- If `self.repeated_system_message` matches `content` **and**
  `entry_index_provider.current() == entry_index + 1` (nothing else allocated since):
  increment `count`, build a `NormalizedEntry` with content
  `format!("{content} {}", "✓".repeat(count - 1))`, latest `metadata`, and return
  `ConversationPatch::replace(entry_index, entry)`.
- Otherwise: `let idx = entry_index_provider.next()`, store
  `Some(RepeatedSystemMessage { entry_index: idx, content: content.clone(), count: 1 })`,
  and return `ConversationPatch::add_normalized_entry(idx, entry)` with the plain content.

## Step 3 — Use the helper in the catch-all branches

In `normalize_entries`, `ClaudeJson::System` match (~line 1400):
- `Some(subtype)` arm: replace the inline entry construction with
  `patches.push(self.push_collapsible_system_message(format!("System: {subtype}"), Some(raw_json), entry_index_provider))`.
- `None` arm: same, with content `"System message"`.

No other branch changes; interruption detection is entirely via the
`current() == entry_index + 1` guard (the provider is shared by all patch producers,
including stderr normalization).

## Step 4 — Unit tests

In the existing `#[cfg(test)]` module of `claude.rs`:

1. `test_repeated_unknown_system_subtype_collapses_with_ticks` — feed four
   `{"type":"system","subtype":"thinking_tokens"}` lines through the processor; assert:
   one add patch at index i, then replace patches at index i; final content
   `System: thinking_tokens ✓✓✓`.
2. `test_repeated_system_subtype_interrupted_starts_new_entry` — thinking_tokens,
   assistant text message, thinking_tokens → two separate entries, both without ticks.
3. `test_different_system_subtypes_do_not_collapse` — `thinking_tokens` then
   `some_other_subtype` → two separate add patches.

Reuse the test-harness style already present (e.g. how `test_thinking_content` and the
init-message tests drive `normalize_entries` / `process_logs`).

## Step 5 — Verify

- `cargo test -p executors` (or the claude module filter) — new + existing tests pass.
- `cargo clippy -p executors` clean.
- `pnpm run format` (repo requirement before completing).
- `pnpm run backend:check` if time allows (full workspace check).

## Step 6 — Pipeline follow-ups

- Codex review of the diff; address confirmed findings (stage 4).
- Seed `docs/knowledge-base/` with topic pages + index, tagged `4095-thinking-tokens`,
  and commit it (stage 5).
- Commit and open PR against `main` (stage 6).

## Risks / notes

- **Streaming replaces don't advance the provider**: an in-flight assistant delta that
  replaces an *earlier* index won't break a tick run. Accepted (see spec) — the collapsed
  entry is still the last visible line.
- **Unbounded tick string**: a very long run produces a long single line; still strictly
  better than N lines. No cap for now.
- Existing single-occurrence behavior stays byte-identical, so no existing test should
  need modification (only additions).
