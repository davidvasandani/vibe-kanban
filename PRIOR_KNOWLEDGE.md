# Prior Knowledge: Concatenating Repeating Lines

The project knowledge base is populated. The most relevant pages are:

- `docs/knowledge-base/collapsing-repeated-log-entries.md`
- `docs/knowledge-base/claude-log-normalization.md`

## Distilled Guidance

1. Collapse repeats in the server-side log normalizer so local, remote, desktop,
   and mobile consumers all receive the same compact patch stream.
2. Only collapse uninterrupted runs. The shared `EntryIndexProvider` gives a
   reliable adjacency check: a stored entry at `i` is still last when
   `current() == i + 1`.
3. Emit an add for the first occurrence and replacements at the original index
   for later repeats. Keep the first occurrence byte-compatible.
4. Bound the indicator. Inline ticks are acceptable for short runs; use `✓ ×N`
   beyond the threshold to avoid memory growth during long loops or replay.
5. Tool calls require lifecycle-aware state:
   - retain every unique tool-call ID for result correlation;
   - only the latest occurrence may update a shared visible row;
   - re-emission of one ID is an update, not a repeat;
   - a prior occurrence must finish successfully before its row is reused;
   - failure must remain visibly failed and must not gain a success tick.
6. Clear any index-bearing repeat tracker whenever entry indices reset or
   historical replay is abandoned.
7. Diagnose the producer of the visible row before changing code. Existing
   compaction is deliberately narrow: Claude's frequent catch-all system events
   and repeated Claude `Bash` calls invoking Grok. The reported
   `codex review --uncommitted` rows are not covered by that Grok-specific path.
8. Unit tests should inspect both patch indices and add/replace operation kinds,
   in addition to rendered content and status.

## Implication for This Task

Reuse the established server-normalizer pattern, but apply it at the component
that owns Codex command-execution rows. Do not broaden Claude's Grok predicate or
introduce frontend deduplication merely because the screenshots show the final
rendered label.
