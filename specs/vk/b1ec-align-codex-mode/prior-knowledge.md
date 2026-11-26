# Prior knowledge: Align Codex model catalog

Sources searched (read-only): `wiki/INDEX.md`, `wiki/executor-model-catalogs.md`,
`wiki/model-picker-preferences.md`, `wiki/codex-credential-refresh.md`,
`specs/vk/3ffa-upgrade-codex-mo/*` (PR #404),
`.specify/memory/constitution.md` (PR #404 review note).

## Applicable knowledge

- **Single source of truth.** The Codex picker is the ordered
  `ModelSelectorConfig.models` vector returned by `Codex::discover_options` in
  `crates/executors/src/executors/codex.rs`. The frontend must not duplicate
  it (`executor-model-catalogs.md`).
- **Exact ordered regression test.** `discovered_models_match_current_chatgpt_catalog`
  asserts IDs, labels and reasoning option IDs in order. Any catalog change
  updates that contract on purpose.
- **Exact menus replace, they don't append.** When the reference is an exact
  current menu, remove superseded entries. Retained entries must still be in
  the current selector.
- **`model/list` is the capability source.** Initialize app-server, call
  `model/list` with `includeHidden: true`, advertise visible entries only, and
  derive reasoning efforts from each record. Hidden here: `gpt-reserve`,
  `codex-auto-review`.
- **Label style.** VK uses spaced labels (`GPT-6 Astra`), while app-server uses
  hyphens (`GPT-6-Astra`). The ID is the contract.
- **Removing entries never strands saved selections.** Launches pass the model
  string through unchanged (constitution principle L).
- **Don't bundle a CLI bump.** The npx `@openai/codex` pin is Renovate-managed.
  The deployment overrides it with `VIBE_CODEX_COMMAND` (homelab `pkgs.codex`).
- **PR #404 lesson (now superseded).** The 0.155.1 `model/list` was treated as
  the compatibility authority, and GPT-6.2 was skipped as "API-only". This task
  makes the user-facing current selector the target instead.
- **Credentials.** Probes must not disturb the shared `auth.json` rotation.
  Use a scoped `CODEX_HOME` with `auth.json` symlinked, as the worker does
  (`codex-credential-refresh.md`).

## New fact surfaced by this task

The ChatGPT backend filters both the catalog and model acceptance by Codex
client version. CLI 0.155.1 gets `model_not_found` /
`model_not_in_accessible_catalog` for gpt-6.2-* and gpt-6.1-*, while 0.171.0
lists and runs them. See `technical-spec.md`.
