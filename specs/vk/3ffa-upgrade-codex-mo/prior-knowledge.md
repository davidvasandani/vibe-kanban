# Prior Knowledge: Upgrade Codex Models

Task: `3ffa-upgrade-codex-mo`.

The project knowledge base is populated. This read-only recall searched both
`wiki/` and `docs/knowledge-base/` and found two directly relevant pages.

## Executor discovery owns the picker

Source: `wiki/executor-model-catalogs.md`.

The authoritative Codex menu is the ordered `ModelSelectorConfig.models`
vector returned by `StandardCodingAgentExecutor::discover_options`; the
frontend intentionally has no duplicate catalog. Catalog changes must be
newest-first and must retain the separate infrastructure-provided Default
choice rather than adding a synthetic default model.

**Consequence.** This change belongs only in the Codex executor discovery
response and its regression test. No frontend or shared-schema edit is needed.

## The deployed app-server is a capability authority

Source: `wiki/executor-model-catalogs.md`.

Provider-facing identifiers and reasoning boundaries must be verified against
current authoritative sources. For Codex, app-server `model/list` is valuable
because it reports visible/hidden state and supported reasoning efforts. Public
OpenAI API documentation can announce models before an older pinned CLI lists
them.

**Consequence.** The pinned Codex 0.155.1 `model/list` result, not the newer
API-only GPT-6.2 family, defines this refresh. It reports six visible entries:
GPT-6 Astra/Sol/Luna and GPT-5.6 Sol/Terra/Luna. Hidden internal models must not
be advertised.

## A supplied/current picker is a replacement contract

Source: `wiki/executor-model-catalogs.md`.

When current menu evidence is exact, superseded entries absent from that menu
should be removed rather than retained additively. The exact ordered catalog,
labels, and reasoning IDs need a focused regression test. Stored selections do
not need a migration because execution passes model strings through.

**Consequence.** Remove GPT-5.5 and GPT-5.3 Codex Spark from discovery, add
GPT-6 Sol and GPT-6 Luna, and correct GPT-6 Astra to include `ultra`.

## Keep CLI and catalog refreshes separate

Sources: `wiki/executor-model-catalogs.md` and
`docs/knowledge-base/executor-model-catalog-maintenance.md`.

Model-menu work should not hand-bump the Renovate-managed Codex CLI. Dependency
upgrades have a larger protocol and deployment blast radius. Verification for
a catalog-only Rust change consists of focused executor tests, generated-type
checks, Rust formatting, and diff inspection; unrelated generated drift should
not be committed.

**Consequence.** Do not change the CLI pin or homelab deployment module. Reuse
the deployed binary read-only for evidence, then keep implementation confined
to `crates/executors/src/executors/codex.rs` plus pipeline and knowledge
artifacts.
