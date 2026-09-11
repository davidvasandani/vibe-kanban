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

## Contributed by

- `vk/094a-update-chatgpt-m`
