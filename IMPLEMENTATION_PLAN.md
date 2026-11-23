# Implementation plan: Restore Opus 4.8 in the Claude Code picker

All edits are in `crates/executors/src/executors/claude.rs`.

1. **Catalog entry.** In `default_discovered_options`, insert
   `("claude-opus-4-8", "Opus 4.8")` right after `("claude-opus-5", "Opus 5")`.
   `supports_effort` already returns true for every ID except Haiku 4.5, so
   Opus 4.8 gets all five effort options with no predicate change.
2. **Context window.** Add `"claude-opus-4-8"` to the 1M arm of
   `context_window_for_model`, after `"claude-opus-5"`.
3. **Tests.**
   - `test_claude_discovered_catalog_is_versioned_and_defaults_to_opus_5_5`:
     expected array grows from 9 to 10 entries, with
     `("claude-opus-4-8", "Opus 4.8", &efforts)` after Opus 5.
   - Context-window test: move `claude-opus-4-8` from the stale 200K assertion
     into the 1M assertions.
   - `test_seeded_1m_window_not_downgraded_by_reported_name` uses
     `claude-opus-4-8` as a name that reports without `[1m]` but should not
     downgrade. It still passes, since 1M stays 1M. Keep it as is.
   - Alias test: unchanged (every alias target is still advertised).
4. **Generated types.** `ModelSelectorConfig` shape is unchanged; run
   `pnpm run generate-types:check` to confirm.
5. **Verify.** `cargo test -p executors`, `pnpm run format`,
   `cargo clippy -p executors`.
6. **Knowledge.** Record in `wiki/executor-model-catalogs.md` that the CLI's
   refusal fallback targets `claude-opus-4-8`, and that catalog rebuilds must
   not silently drop explicit models.
