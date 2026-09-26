# SPEC: Model menu fixes (mobile keyboard, Opus 5.5 default, versioned names, disabling models)

Task: `vk/6823-model-menu`

## Problem

User report (iPhone screenshots of the chat composer's model picker):

1. **Hidden behind keyboard.** Opening the model menu immediately raises the
   iOS software keyboard, which covers almost the entire menu. Cause:
   `DropdownMenuSearchInput` (`packages/ui/src/components/Dropdown.tsx`) is
   rendered with `autoFocus`, and `ModelSelectorPopover` shows it whenever the
   list has more than 8 models (Claude has 9).
2. **Claude default should be Opus 5.5.** The Claude Code catalog
   (`crates/executors/src/executors/claude.rs::default_discovered_options`)
   defaults to the `opus` alias.
3. **Models should always include a version.** The Claude catalog advertises
   unversioned aliases — `Opus`, `Opus (1M context)`, `Sonnet`, `Fable`,
   `Haiku` — next to their versioned equivalents.
4. **Option to disable models.** There is no way to hide models a user never
   uses from the picker.

## Requirements

### R1 — No keyboard on open (touch devices)
- On mobile — the responsive layout (`useIsMobile()`) **or** a physical
  mobile device (`isRealMobileDevice()`, the heuristic the command-bar
  `SelectionDialog` already uses) — opening the model menu must not focus
  the filter input. The filter stays available; the keyboard only appears when
  the user taps it.
- Desktop behaviour is unchanged (filter still auto-focuses).
- `DropdownMenuSearchInput` keeps `autoFocus` as its default so other menus are
  unaffected; the model popover opts out through a new `autoFocusSearch` prop.

### R2 — Opus 5.5 is the Claude default
- `default_model` becomes `claude-opus-5-5`. The implicit preset model
  (`get_preset_options`) follows automatically.

### R3 — Versioned Claude catalog
Ordered catalog (newest first, IDs are CLI-accepted explicit model IDs):

| ID | Label | Effort |
| --- | --- | --- |
| `claude-opus-5-5` | Opus 5.5 | low…max |
| `claude-opus-5` | Opus 5 | low…max |
| `claude-sonnet-5` | Sonnet 5 | low…max |
| `claude-fable-5-1` | Fable 5.1 | low…max |
| `claude-haiku-4-5` | Haiku 4.5 | none |

- Aliases `opus`, `opus[1m]`, `sonnet`, `fable`, `haiku` are removed from the
  advertised catalog. `opus[1m]` is redundant: Opus 5.5 already has a 1M window.
- Backward compatibility: saved profiles/overrides using an alias still launch
  (the CLI accepts aliases; `context_window_for_model` keeps alias handling),
  and `appendPresetModel` still surfaces a preset's non-catalog model.
- Exact ordered regression test over IDs, labels and effort IDs (wiki:
  executor-model-catalogs).
- Other executors already carry versions in every label; no change.

### R4 — Disable models per agent
- New persisted field `disabled_models: Vec<String>` on `ExecutorProfile`
  (profiles.json, per executor; model keys use the picker's key format
  `provider/id` or `id`, matched case-insensitively). Serde default/skip when
  empty so existing files are unaffected; merged in
  `merge_with_defaults`/`compute_overrides` like `recently_used_models`.
- The picker hides disabled models, **except** the currently selected model
  (so the trigger never shows a model missing from its own menu).
- Settings → Agents gets a **Models** card for the selected agent listing its
  discovered models with a checkbox each. Unchecking disables. At least one
  model must remain enabled. Changes go through the existing dirty/save bar.
- `disabled_models` is a reserved key for variant enumeration
  (`getExecutorVariantKeys`).

## Non-goals
- Re-positioning the popover above the keyboard when the user explicitly taps
  the filter (visual-viewport-aware popper) — out of scope; R1 removes the
  unrequested keyboard, and R4 lets users shorten the list below the 8-model
  search threshold.
- Changing the Claude CLI pin or other executors' catalogs.
- Homelab/deployment changes.

## Acceptance
- iPhone: tapping the model trigger opens the menu with no keyboard.
- New Claude sessions default to "Opus 5.5 · High" style label with model
  `claude-opus-5-5`.
- Claude menu lists exactly the 5 versioned models.
- Disabling e.g. Haiku 4.5 in Settings removes it from the chat picker; the
  setting survives reload.
- `cargo test -p executors`, `pnpm run check`, `pnpm run lint`, Vitest for new
  helpers pass; `pnpm run generate-types` updated.
