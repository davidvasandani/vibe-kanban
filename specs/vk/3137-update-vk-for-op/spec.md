# Feature Spec: Add Claude Opus 5 to Executor Model Selectors

## Summary

Add Claude Opus 5 (`claude-opus-5`) to the hard-coded model catalogs of every
executor that already carries Claude model entries. Users should be able to
select Opus 5 from the model picker when using any supported executor. Existing
models and aliases must remain unchanged.

## Motivation

Anthropic has released Claude Opus 5 with the canonical API model ID
`claude-opus-5`. Vibe Kanban maintains hard-coded model catalogs per executor
so users can select models from the UI. Without this change, Opus 5 is
unavailable in the model picker even though the backing agent CLIs support it.

## Affected Executors

Four executors maintain hard-coded Claude model entries today. Each uses a
provider-specific naming convention that must be followed:

### 1. Claude Code (`claude.rs`)

Uses a mix of short alias names consumed by the `claude` CLI (e.g. `"opus"`,
`"sonnet"`, `"haiku"`) **and** versioned model IDs for newer releases (e.g.
`"claude-sonnet-5"` alongside the `"sonnet"` alias).

- **Current Opus entries:** `"opus"`, `"opus[1m]"`
- **Precedent:** `"claude-sonnet-5"` (display label `"Sonnet 5"`) was added
  alongside the `"sonnet"` alias, proving the catalog carries versioned IDs
  for new major Claude releases even when an alias exists.
- **New entry:** `"claude-opus-5"` with display label `"Opus 5"`
- **Placement:** After `"opus[1m]"` and before `"claude-sonnet-5"` (grouping
  Opus entries together, newest versioned ID adjacent to the aliases it
  supplements).
- **Reasoning options:** The existing `supports_effort` closure
  (`id.contains("opus")`) automatically covers `"claude-opus-5"`. No
  additional reasoning-mode change required.

### 2. Cursor (`cursor.rs`)

Uses dot-separated identifiers without a `claude-` prefix (e.g. `"opus-4.8"`,
`"opus-4.6"`, `"sonnet-4.6"`). Display labels follow the pattern
`"Claude {version} {family}"` (e.g. `"Claude 4.8 Opus"`). Reasoning mode
resolution maps each model to standard and thinking variants.

- **New model ID:** `"opus-5"`
- **Display label:** `"Claude 5 Opus"`
- **Reasoning resolution:** Add two match arms:
  `("opus-5", Some("standard"))` → `"opus-5"` and
  `("opus-5", Some("thinking") | None)` → `"opus-5-thinking"`.
- **Reasoning-mode options:** Add `"opus-5"` to the existing Claude match arm
  in `cursor_reasoning_options` alongside `"opus-4.8"`, `"opus-4.6"`, etc.
- **Description string:** Insert `opus-5` right after `auto` in the
  `#[schemars(description = "...")]` annotation (before `opus-4.8`).
- **Catalog placement:** Insert before `("opus-4.8", "Claude 4.8 Opus")`.

### 3. Copilot (`copilot.rs`)

Uses dot-separated identifiers with a `claude-` prefix (e.g.
`"claude-opus-4.8"`, `"claude-opus-4.6"`). Display labels follow the pattern
`"Claude {family} {version}"` (e.g. `"Claude Opus 4.8"`).

- **New model ID:** `"claude-opus-5"`
- **Display label:** `"Claude Opus 5"`
- **Placement:** Insert before `("claude-opus-4.8", "Claude Opus 4.8")`.

### 4. Droid (`droid.rs`)

Uses Anthropic's hyphenated API model IDs (e.g. `"claude-opus-4-8"`,
`"claude-opus-4-6"`). Display labels follow the pattern
`"Claude {family} {version}"` (e.g. `"Claude Opus 4.8"`).

- **New model ID:** `"claude-opus-5"`
- **Display label:** `"Claude Opus 5"`
- **Description string:** Add `claude-opus-5` to the `#[schemars(description)]`
  examples list.
- **Placement:** Insert before `("claude-opus-4-8", "Claude Opus 4.8")`.

**Note on Copilot/Droid ID convergence:** Copilot uses dots (e.g.
`claude-opus-4.6`) while Droid uses hyphens (e.g. `claude-opus-4-6`). For
Opus 5, both conventions produce the identical string `"claude-opus-5"` because
the version `5` has no decimal point to differentiate.

## Executors NOT Affected

- **Amp, Codex, Gemini, Grok, OpenCode, Qwen:** Do not carry Claude model
  entries in their catalogs.

## Requirements

1. Each new entry must use the provider-specific naming convention established
   by existing entries in that executor.
2. Existing model entries and aliases must not be removed or reordered
   (except that the new entry is inserted in "newest first" position).
3. Cursor reasoning-mode resolution must correctly produce both standard and
   thinking variants for the new model.
4. Default model selections must not change.

## Generated Artifacts

If any schema-carrying metadata (e.g. CLI `--model` description strings)
changes, the corresponding generated schemas and TypeScript types must be
regenerated using the repository's documented commands:

- `pnpm run generate-types` (for `shared/types.ts`)
- Schema generation for `shared/schemas/` if executor schema metadata changes

Generated files must not be edited by hand.

## Testing

- Add or extend focused Rust unit tests for each changed executor to verify:
  - The new model appears in the catalog/discovery output.
  - Cursor reasoning-mode resolution produces correct standard and thinking
    identifiers for Opus 5.
- Run `cargo test --workspace` to verify no regressions.
- Run `pnpm run generate-types:check` to verify type generation is consistent.
- Run `pnpm run format` and `pnpm run lint` before finalizing.

## Non-Goals

- Changing default model selections for any executor.
- Removing or deprecating older Claude models.
- Bumping agent CLI package versions (managed by Renovate).
- Adding fast-mode variants without evidence of provider support. (Fast mode
  currently exists only for specific Opus 4.6 entries in Copilot and Droid;
  no Opus 4.8 or Opus 5 fast-mode entries exist or are proposed.)
- Modifying the Claude Code executor's existing alias-based entries (`"opus"`,
  `"opus[1m]"`, `"sonnet"`, `"haiku"`, `"fable"`).

## Clarifications

Ambiguities identified during spec review and their resolutions:

### C1. Claude Code is NOT alias-only — versioned ID required

**Ambiguity:** The original spec stated that Claude Code uses only CLI-resolved
aliases and therefore needs no change. The root-level SPEC.md contradicted this,
requiring "an explicit Opus 5 selection."

**Evidence:** The current Claude Code catalog (`claude.rs:281-288`) contains
`("claude-sonnet-5", "Sonnet 5")` alongside the `"sonnet"` alias. This proves
the executor adds versioned model IDs for new major Claude releases even when
an alias exists for the model family.

**Resolution:** Claude Code **does** need a new entry: `("claude-opus-5",
"Opus 5")`. This follows the `claude-sonnet-5` precedent exactly. The existing
`"opus"` and `"opus[1m]"` aliases remain unchanged. The `supports_effort`
closure (`id.contains("opus")`) automatically grants reasoning options to the
new entry — no additional logic change needed.

### C2. Cursor description string insertion point

**Ambiguity:** The spec said to add `opus-5` to the description string but
didn't specify where.

**Resolution:** Insert `opus-5` immediately after `auto` (before `opus-4.8`)
to maintain the newest-first ordering. Resulting prefix:
`"auto, opus-5, opus-4.8, opus-4.6, ..."`.

### C3. Copilot/Droid model ID convention convergence

**Ambiguity:** Copilot uses dots in version numbers (`claude-opus-4.6`) while
Droid uses hyphens (`claude-opus-4-6`). Which convention applies to Opus 5?

**Resolution:** Both conventions produce the same string `"claude-opus-5"` for
an integer version. No ambiguity in practice, but implementers should be aware
that this convergence is version-specific, not a convention change.

### C4. No fast-mode variants for Opus 5

**Ambiguity:** Copilot has `claude-opus-4.6-fast` and Droid has
`claude-opus-4-6-fast`, but neither has fast mode for Opus 4.8. Should Opus 5
get fast-mode entries?

**Resolution:** No. Fast mode was added only for specific 4.6 entries. There is
no evidence of provider support for Opus 4.8 or Opus 5 fast mode. This is
explicitly listed as a non-goal.

### C5. SPEC.md requirement 1 alignment

**Ambiguity:** SPEC.md requirement 1 stated "Claude Code must offer an explicit
Opus 5 selection" while the original detailed spec excluded Claude Code.

**Resolution:** SPEC.md was correct. The detailed spec has been updated (see C1)
to include Claude Code. Both documents now agree.

## References

- SPEC.md and PRIOR_KNOWLEDGE.md in the repository root for broader context.
- Anthropic model overview: https://docs.anthropic.com/en/docs/about-claude/models
