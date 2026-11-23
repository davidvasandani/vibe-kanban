# Prior knowledge: Restore Opus 4.8 in the Claude Code picker

Distilled from the project knowledge base (`wiki/`). Read-only recall for the
spec and plan stages.

## wiki/executor-model-catalogs.md

- Executor discovery (`ModelSelectorConfig.models`) is the only source of truth
  for the picker. The frontend keeps no duplicate list.
- Advertise explicit, versioned IDs only, newest-first. Keep earlier explicit
  models unless removal was requested. Opus 4.8 was dropped without a request
  when the catalog was rebuilt (#329/#397), so restoring it follows this rule.
- Cover the catalog with an exact, ordered regression test over IDs, labels and
  reasoning-option IDs, so any catalog change updates the contract on purpose.
- Derive capabilities and context windows from the pinned binary's embedded
  catalog (`capabilities`, `context.window`), not from name substrings. The
  2.1.322 record for `claude-opus-4-8` shows a 1M native window and all five
  effort levels.
- The catalog, the context mapping and the launcher are separate contracts in
  `claude.rs`. Adding a model to the catalog means updating
  `context_window_for_model` too.
- Keep catalog changes apart from CLI pin bumps. `@anthropic-ai/claude-code` is
  a `needs-review` Renovate carve-out.
- Aliases map to advertised models through `model_aliases`. Opus 4.8 is not an
  alias target in 2.1.322 (`opus` → `claude-opus-5-5`), so the alias map stays
  unchanged.

## wiki/model-picker-preferences.md

- `disabled_models` is a per-agent denylist, so a newly advertised model shows
  by default. Hidden-model filtering applies only to the popover config.
  The current selection always stays visible.
- The screenshot showing five models reflects this user's disabled models, not
  the catalog size (the catalog has nine).

## Background found during investigation (not in wiki yet)

- The pinned CLI routes Opus 5.5 `cyber` refusals to `claude-opus-4-8` itself
  (hard-coded map). VK cannot influence this, so a turn can show a
  `claude-opus-4-8` 404 that VK never sent.
