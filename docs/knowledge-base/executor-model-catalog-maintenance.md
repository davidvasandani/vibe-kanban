# Executor model catalog maintenance

Vibe Kanban maintains fallback model catalogs in each executor because several
agent CLIs do not expose a complete machine-readable model list. A new model
therefore needs a cross-executor audit rather than one global registry edit.

## Update procedure

1. Confirm the canonical model ID in first-party vendor documentation.
2. Search `crates/executors/src/executors/` for the previous model generation.
   Provider identifiers may differ: Claude Code, Copilot, and Droid use
   `claude-opus-5`, while Cursor uses `opus-5`.
3. Update each supported executor's `discover_options()` catalog. Claude Code's
   fallback catalog lives in `default_discovered_options()`.
4. Update executor-specific resolution logic. Cursor maps its base model plus
   reasoning choice to separate standard and `-thinking` identifiers.
5. Update any `schemars` model-description strings and run
   `pnpm run generate-types`; do not hand-edit generated schemas.
6. Add focused tests for catalog presence and any provider-specific name or
   reasoning resolution.
7. Check release-specific metadata outside the catalog. Context-window
   inference is one example: Claude Opus 5 uses a 1M-token window without the
   older `[1m]` suffix.

Generated schema changes should be limited to executors whose source schema
descriptions changed. If `shared/types.ts` changes for an unrelated pre-existing
source drift, keep that unrelated generated change out of the task.

## Verification

- Run focused executor tests.
- Run `pnpm run generate-types:check`.
- Run Rust formatting and `git diff --check`.
- Inspect the complete diff for provider-name consistency and unrelated
  generated artifacts.

## Contributed by

- `3137-update-vk-for-op`
