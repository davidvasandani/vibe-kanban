# Implementation Plan: Restore Opus 4.8 in the Claude Code model picker

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context
Rust, `crates/executors`. The Claude Code picker is populated from
`default_discovered_options()` in `crates/executors/src/executors/claude.rs`
(the `ModelSelectorConfig` that `discover_options` streams to the frontend).
The frontend (`packages/web-core/src/shared/components/ModelSelectorContainer.tsx`)
renders that catalog and applies the `disabled_models` display filter. It has
no model list of its own. Constraint: the Claude Code CLI pin (2.1.322) is a
`needs-review` Renovate carve-out and is not touched.

## Architecture & Approach
Data-only change in one file, `crates/executors/src/executors/claude.rs`:

1. FR-1/FR-2: in `default_discovered_options()`, insert
   `("claude-opus-4-8", "Opus 4.8")` after `("claude-opus-5", "Opus 5")`.
2. FR-3: no code change. `supports_effort` returns true for every ID except
   `claude-haiku-4-5`, so the five effort options are attached automatically.
3. FR-4: add `"claude-opus-4-8"` to the 1M arm of `context_window_for_model()`.
   The log processor seeds the gauge from the configured model
   (`claude.rs` ~1745) and also re-checks the model reported by `init`/assistant
   messages (~2267, ~3153), so an Opus 4.8 session measures against 1M.
4. FR-5: `default_model` and `model_aliases` are not edited.
5. FR-6: no code change. `disabled_models` is a denylist
   (`packages/web-core/src/shared/lib/disabledModels.ts`).

Tests (same file, `#[cfg(test)]`):
- `test_claude_discovered_catalog_is_versioned_and_defaults_to_opus_5_5`:
  the expected array goes from `[_; 9]` to `[_; 10]` with Opus 4.8 after Opus 5.
- `test_context_window_for_model`: replace the stale 200K assertion for
  `claude-opus-4-8` with a 1M assertion.
- `test_claude_legacy_aliases_resolve_to_catalog_models`: unchanged, must pass.

## Data Model
Not applicable. No new type and no change to `ModelSelectorConfig` / `ModelInfo`
(so `shared/types.ts` is unchanged).

## Contracts
Not applicable. The discover-options response shape is unchanged; only its data
gains one entry.

## Research Notes
See `./research.md`.

## Constitution Check
- II (Test the contract): ordered catalog test and context-window test updated.
- VI (Don't rebuild what shipped): reuses the existing catalog, effort predicate
  and disabled-models filter. Nothing new is built.
- IX (External agent protocols are defensive contracts): ID, label, effort
  levels and window are read from the pinned binary's catalog record, not
  inferred from the name.
- VIII / Dependencies: no CLI pin change.
- L (Hiding a choice never strands a selection): the new entry goes through the
  existing filter. Nothing about hiding changes.
No deviations.

## Risks & Dependencies
- Accounts without Opus 4.8 access get the provider's model error when they pick
  it. This is accepted, the same as for Mythos 5.2 (spec, Out of Scope).
- A future CLI bump that drops `claude-opus-4-8` from its catalog would need a
  catalog update. The wiki's CLI-upgrade checklist covers re-reading the catalog.
