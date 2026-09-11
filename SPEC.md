# Technical Specification: Refresh ChatGPT Model Catalog

## Objective

Update Vibe Kanban's Codex executor model selector to match the current
ChatGPT/Codex model menu shown in the supplied product reference. The selector
must expose the current named models in the same newest-first order and stop
advertising superseded entries that are absent from that menu.

## Required catalog

The Codex executor must advertise these explicit model choices, in order:

1. `gpt-6-astra` — **GPT-6 Astra**
2. `gpt-5.6-sol` — **GPT-5.6 Sol**
3. `gpt-5.6-terra` — **GPT-5.6 Terra**
4. `gpt-5.6-luna` — **GPT-5.6 Luna**
5. `gpt-5.5` — **GPT-5.5**
6. `gpt-5.3-codex-spark` — **GPT-5.3 Codex Spark**

The UI's separate **Default** choice is supplied by the model-selector
infrastructure and is not an explicit model entry.

## Behavioural requirements

- Preserve the established reasoning-effort support for retained models.
- Assign reasoning options to new models according to the capabilities exposed
  by the current Codex CLI/model family, and cover those assignments with a
  focused regression test.
- Do not change permissions, slash commands, defaults, authentication, or
  executor launch behaviour.
- Keep model IDs and display labels centralized in the Codex executor's
  discovery response; no frontend-specific duplicate catalog is required.

## Verification

- Focused Codex executor unit tests prove the exact ordered IDs, labels, and
  reasoning options.
- Repository formatting and relevant Rust checks/tests pass.
- Generated artifacts remain unchanged unless the source change legitimately
  requires regeneration.

## Scope

Only the `vibe-kanban` repository requires a product-code change. The sibling
`homelab` repository is not part of the implementation or pull request.
