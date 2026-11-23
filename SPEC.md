# SPEC: Restore Claude Opus 4.8 in the Claude Code model picker

## Problem

The Claude Code model picker no longer offers Opus 4.8. The executor's
discovered catalog (`default_discovered_options` in
`crates/executors/src/executors/claude.rs`) was rebuilt as a versioned list in
#329 and refreshed in #397, and `claude-opus-4-8` was never carried into it.
Users who want to run Opus 4.8 cannot select it.

Related symptom (context, not the fix): when Opus 5.5 refuses a `cyber`-category
request, the pinned Claude CLI retries on `claude-opus-4-8`. If the account
cannot reach that model, the turn fails with "There's an issue with the
selected model (claude-opus-4-8)". VK does not choose this fallback model and
cannot change it. This task only restores explicit selection.

## Evidence

- The pinned CLI (`@anthropic-ai/claude-code@2.1.322`, native artifact sha256
  `bfb3703ad5380242328459bc62ada4bde8ae7ba57c6e1eeadb9c00e08874c478`) lists
  `{id:"claude-opus-4-8", family:"opus", display_name:"Opus 4.8"}` with
  `context.window: 1e6`, `native_1m: true`, capabilities `effort`,
  `xhigh_effort`, `max_effort`, `adaptive_thinking`, and `default_effort: "high"`.
- Anthropic's model reference lists `claude-opus-4-8` as Active with a 1M context
  window.

## Requirements

- R1: The Claude Code catalog includes `("claude-opus-4-8", "Opus 4.8")`,
  placed newest-first right after `claude-opus-5` (Opus 4.8 is older than Opus 5).
- R2: Opus 4.8 offers the same effort options as other current models:
  `low`, `medium`, `high`, `xhigh`, `max`.
- R3: `context_window_for_model("claude-opus-4-8")` returns 1,000,000, matching
  the CLI catalog. The current unit test asserting 200K is stale and is updated.
- R4: The default model (`claude-opus-5-5`) and the alias map are unchanged.
  No alias points at Opus 4.8.
- R5: The ordered catalog regression test is updated to include the new entry.
- R6: No CLI pin change and no frontend change. The picker renders the backend
  catalog. `disabled_models` is a denylist, so the new entry shows by default.

## Out of scope

- Changing the CLI's refusal-fallback routing, or setting
  `CLAUDE_CODE_DISABLE_REFUSAL_FALLBACK`.
- Account-side access to Opus 4.8.
- Other executors (Cursor, Copilot, Droid already list Opus 4.8).

## Acceptance

- `cargo test -p executors` passes, including the updated catalog and
  context-window tests.
- `pnpm run generate-types:check`, format and lint pass.
