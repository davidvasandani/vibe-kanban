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

## Claude Code dependency refreshes

Claude Code is a special dependency carve-out because a passing build cannot
prove model aliases or Vibe Kanban's imposed safety controls. For an explicitly
requested, manually reviewed refresh:

1. Compare npm's `latest`, `stable`, and `next` channels and pin the intended
   immutable version. Do not mistake `next` for the generally published latest.
2. Review Anthropic's changelog from the old pin through the new pin for
   noninteractive protocol, model, permission-hook, and alias changes.
3. Extract the relevant platform-native package at the selected version. The
   npm wrapper's `sdk-tools.d.ts` contains schema titles, not necessarily the
   wire names used by permission rules.
4. Confirm the native catalog's exact model IDs, aliases, effort capabilities,
   context windows, and canonical/legacy tool-name map. Update every deliberate
   version twin in source comments and tests.
5. Audit model metadata as a cross-product, not one new row in isolation. If a
   newly current model has native 1M context, both its explicit ID and retained
   family alias need the same context classification. Check the other current
   family entries at the same time; this catches stale alias behavior that an
   explicit-ID-only test misses.
6. Preserve parameter-level controls at their real enforcement point. Claude's
   background path still requires inspecting `Bash.run_in_background`; a list
   of denied background tool names cannot replace that guard.
7. Verify the deployment's Node runtime against the wrapper's `engines` field,
   even when the executor launches through `npx`. A compatible host-installed
   `claude-code` package does not by itself prove the npm wrapper can start.

For Claude Code 2.1.268, the native catalog identifies Fable 5.1 as
`claude-fable-5-1` with `low`, `medium`, `high`, `xhigh`, and `max` effort and a
native 1M-token context. The current `opus`, `sonnet`, and `fable` aliases and
their explicit Opus 5, Sonnet 5, and Fable 5.1 IDs all require native-1M context
classification; Haiku remains on the default window unless `[1m]` is explicit.

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
- `vk/1f95-update-claude-mo`
