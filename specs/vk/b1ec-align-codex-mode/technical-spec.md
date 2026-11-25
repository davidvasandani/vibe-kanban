# Technical Spec: Align Codex model catalog with the current selector

Task: `vk/b1ec-align-codex-mode` (follow-up to PR #404, `vk/3ffa-upgrade-codex-mo`)

## Goal

Make `Codex::discover_options` (`crates/executors/src/executors/codex.rs`)
advertise exactly the models in the current Codex selector, with each model's
supported reasoning efforts. Hidden models stay out. The exact ordered
regression test `discovered_models_match_current_chatgpt_catalog` must be
updated to match.

## Evidence collected (2026-11-23)

### `model/list` (app-server, `includeHidden: true`)

Current Codex CLI **0.171.0** (npm `latest`), scoped `CODEX_HOME` with the
shared `auth.json` symlinked in:

| Backend priority | ID | Label | Hidden | Efforts |
| ---: | --- | --- | --- | --- |
| 1 | `gpt-6.2-sol` (default) | GPT-6.2-Sol | no | low–ultra |
| 2 | `gpt-6.2-astra` | GPT-6.2-Astra | no | low–ultra |
| 3 | `gpt-6.1-terra` | GPT-6.1 Terra | no | low–ultra |
| 4 | `gpt-reserve` | GPT-Reserve | **yes** | low–max |
| 5 | `gpt-6.1-sol` | GPT-6.1-Sol | no | low–ultra |
| 6 | `gpt-6-astra` | GPT-6-Astra | no | low–ultra |
| 7 | `gpt-6-sol` | GPT-6-Sol | no | low–ultra |
| 8 | `gpt-6-luna` | GPT-6-Luna | no | low–max |
| 9 | `gpt-5.6-sol` | GPT-5.6-Sol | no | low–ultra |
| 10 | `gpt-5.6-terra` | GPT-5.6-Terra | no | low–ultra |
| 11 | `gpt-5.6-luna` | GPT-5.6-Luna | no | low–max |
| 12 | `codex-auto-review` | Codex Auto Review | **yes** | low–max |

"low–ultra" = low, medium, high, xhigh, max, ultra. "low–max" omits ultra.

The deployed CLI **0.155.1** (`pkgs.codex`, set as `VIBE_CODEX_COMMAND` in
homelab `modules/vibe-kanban-rebuild.nix`) still gets the older 8-entry list
(the ChatGPT backend filters the catalog by `client_version`). That list has
no GPT-6.2 or GPT-6.1 entries.

### Selector ordering

- The 0.171.0 TUI `/model` picker (driven in a pty) renders backend priority:
  6.2 Sol, 6.2 Astra, 6.1 Terra, 6.1 Sol, 6 Astra, 6 Sol, 6 Luna, 5.6 Sol, …
- The task's selector screenshot orders: 6.2 Astra, 6.2 Sol, 6.1 Sol,
  6.1 Terra, 6 Astra, 6 Sol, 6 Luna. That is version descending, then tier
  Astra > Sol > Terra > Luna. The same rule applied to the part below the
  screenshot gives 5.6 Sol, 5.6 Terra, 5.6 Luna, which matches both sources.

### Runtime acceptance (one-word `codex exec` smoke prompts)

| CLI | Model | Result |
| --- | --- | --- |
| 0.155.1 (deployed) | gpt-6.2-astra, gpt-6.2-sol, gpt-6.1-sol, gpt-6.1-terra | **400 `model_not_found`**: "not supported when using Codex with a ChatGPT account" |
| 0.155.1 (deployed) | gpt-6-astra (control) | OK |
| 0.171.0 | gpt-6.2-astra, gpt-6.1-terra | OK |

## Blocker

The deployed runtime rejects all four new models. If the catalog ships before
the homelab Codex package moves past 0.155.1, the top four dropdown entries
will fail every turn with `model_not_found`. The fix is a homelab deployment
change, which this task explicitly puts out of scope. Implementation is paused
pending an operator decision (see below).

## Proposed catalog (once unblocked)

| # | ID | Label | Efforts |
| ---: | --- | --- | --- |
| 1 | `gpt-6.2-astra` | GPT-6.2 Astra | low–ultra |
| 2 | `gpt-6.2-sol` | GPT-6.2 Sol | low–ultra |
| 3 | `gpt-6.1-sol` | GPT-6.1 Sol | low–ultra |
| 4 | `gpt-6.1-terra` | GPT-6.1 Terra | low–ultra |
| 5 | `gpt-6-astra` | GPT-6 Astra | low–ultra |
| 6 | `gpt-6-sol` | GPT-6 Sol | low–ultra |
| 7 | `gpt-6-luna` | GPT-6 Luna | low–max |
| 8 | `gpt-5.6-sol` | GPT-5.6 Sol | low–ultra |
| 9 | `gpt-5.6-terra` | GPT-5.6 Terra | low–ultra |
| 10 | `gpt-5.6-luna` | GPT-5.6 Luna | low–max |

Labels keep VK's spaced style. Only `codex.rs` (catalog + exact test),
`wiki/executor-model-catalogs.md`, and this spec directory change. No CLI pin
bump (the npx pin is Renovate-managed and the deployment overrides it anyway).

## Open decisions

1. Ship order: wait for the homelab Codex bump (recommended), or ship now and
   accept failing entries until then?
2. Ordering: screenshot order (as specified) or 0.171.0 TUI/backend-priority
   order?

> Update 2026-11-25: decision 2 was resolved in favour of the screenshot order
> (`clarifications.md` Q1). Decision 1 remains open as the merge gate.
