# Executor model catalogs

Vibe Kanban's coding-agent model pickers are populated from each executor's
`StandardCodingAgentExecutor::discover_options` response. For Codex, the
authoritative catalog is the ordered `ModelSelectorConfig.models` vector in
`crates/executors/src/executors/codex.rs`; the frontend consumes that shared
response and should not maintain a duplicate model list.

Model refreshes are contract changes even though the data is hard-coded. Verify
provider-facing IDs and capability boundaries against a current authoritative
source. Codex's app-server `model/list` method is especially useful because it
returns visible/hidden state and supported reasoning efforts, while OpenAI's
official latest-model resolver and model reference cover newly announced models
that an older pinned CLI may not yet list.

Keep the catalog newest-first and distinguish the infrastructure-provided
Default choice from explicit model entries. When a supplied picker reference is
an exact current menu, remove superseded advertised entries rather than treating
the refresh as additive. Cover the result with an exact ordered regression test
over IDs, labels, and reasoning option IDs; this deliberately forces future
catalog changes to update the contract consciously.

Do not bundle a model-menu refresh with a hand-written Codex CLI bump. The CLI
pin is Renovate-managed, and protocol/dependency upgrades have a much larger
blast radius than discovery data.

## Claude CLI upgrades

Claude's launch command, picker, and context-window inference are separate
contracts in `crates/executors/src/executors/claude.rs`. A host Nix package bump
alone does not affect VK's pinned npm launcher. When an explicitly authorized CLI
upgrade adds a model, update that launcher, the explicit catalog entry, and the
context mapping together. Retain earlier explicit models unless removal was
requested. The `opus` alias moved to Opus 5.5 in Claude Code 2.1.280. In
2.1.322, `fable`, `sonnet` and `haiku` moved to Fable 5.2, Sonnet 5.5 and Haiku
5.5 (`specs/vk/7c99-add-new-claude-m/verification.md`).

Before moving the pin, inspect the actual npm native artifact's embedded
JavaScript for timeout environment keys and background-tool wire names. SDK
schema titles are not wire names. Preserve old evidence as historical; document
the new artifact checksum and current checks (`specs/opus-5-5-verification.md`).

## Versioned labels and legacy aliases

Advertise explicit, versioned model IDs only (`claude-opus-5-5` → "Opus 5.5"),
and make the default an explicit ID too. A floating alias such as `opus`
silently changes meaning when the CLI pin moves, so a label like "Opus" never
says what runs. `opus[1m]` is redundant for current models, which already have
1M windows.

Removing aliases from `models` must not strand saved selections. Profiles and
session overrides may still carry `opus` and similar aliases. If the frontend
cannot match that ID, `appendPresetModel` synthesizes an entry with no
reasoning options, and the effort selector disappears. Instead, the executor
publishes `ModelSelectorConfig.model_aliases` (alias → advertised ID), and the
picker resolves saved IDs through `resolveModelAlias` before matching.
Launches still send the alias unchanged. Read the alias map from the pinned
CLI binary itself. Since about 2.1.32x the native artifact embeds a structured
catalog: a JSON-like array of `{id:"claude-…",family:…,context:{window:…},capabilities:[…]}`
records, followed by `aliases:{opus:{default:"claude-…",per_provider:{…}},…}`.
Read the first-party `default`, because VK runs first-party. Guard the map with
a test that fails when the pin moves.

## A CLI bump moves more aliases than the one you came for

Adding Fable 5.2 required Claude Code 2.1.321 or later. In 2.1.322 the same
alias table also moved `sonnet` → `claude-sonnet-5-5` and `haiku` →
`claude-haiku-5-5`. That change wasn't in the request, but a test requires every
alias target to be an advertised model. So any model that a moved alias now
points at has to join the catalog. Pointing the alias at the old model would be
false, and dropping the alias would make saved profiles render without an
effort selector. When you bump, diff the whole alias table, not only the family
you're adding. Then tell the user which shorthand selections changed model.

Derive per-model capabilities from the binary's `capabilities` and
`context.window`, not from name substrings. The old effort predicate
(`contains("opus"|"sonnet"|"fable")`) would have silently denied effort to
`claude-mythos-5-2` and `claude-haiku-5-5`, which both list `effort`,
`xhigh_effort` and `max_effort`. Moving `haiku` to Haiku 5.5 also moved the
alias from the 200K default to a 1M window. Mythos records have `null`
third-party provider IDs (first-party only), and the CLI defines **no** `mythos`
alias. Advertise the versioned ID only, and leave entitlement errors to the API.

## Contributed by

- `vk/094a-update-chatgpt-m`

- `vk/129c-update-to-opus-5`
- `vk/6823-model-menu`
- `vk/7c99-add-new-claude-m`
