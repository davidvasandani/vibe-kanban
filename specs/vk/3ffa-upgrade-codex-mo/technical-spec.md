# Technical Spec: Upgrade Codex Models

## Summary

Update Vibe Kanban's statically discovered Codex model catalog to match the
visible catalog exposed by the deployed Codex 0.155.1 app-server. Keep
unrelated Codex execution, session, deployment, and prompt behavior unchanged.

## Scope

- Change the Codex model selector maintained in
  `crates/executors/src/executors/codex.rs`.
- Advertise the deployed CLI's six visible models, newest-first:
  `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`,
  `gpt-5.6-terra`, and `gpt-5.6-luna`.
- Remove `gpt-5.5` and `gpt-5.3-codex-spark`, which are no longer visible in
  that authoritative picker catalog.
- Assign the exact reasoning-effort options reported by app-server
  `model/list`: Astra, Sol, GPT-5.6 Sol, and GPT-5.6 Terra support through
  `ultra`; Luna and GPT-5.6 Luna support through `max`.
- Update the catalog regression test to pin identifiers, labels, ordering, and
  reasoning options.

## Out of Scope

- Changes to other agent providers or services.
- Changes to the Codex CLI version, transport, authentication, prompts,
  service tiers, or session lifecycle.
- Advertising API-only GPT-6.2 model IDs before the pinned ChatGPT Codex CLI
  exposes them.
- Changes to the homelab deployment module.
- Rewriting historical fixtures whose model strings merely exercise wire
  preservation rather than define the selectable catalog.

## Current State

`Codex::discover_options` returns a hard-coded ChatGPT model selector led by
`gpt-6-astra` and `gpt-5.6-sol`. It omits the visible GPT-6 Sol and GPT-6 Luna
entries, understates GPT-6 Astra's reasoning range, and still advertises two
models absent from the deployed picker. A unit test serializes the discovery
patch and asserts the complete ordered catalog.

## Target Behavior

When Vibe Kanban discovers Codex executor options, users see the same six
visible models and reasoning choices as the deployed Codex app-server. The
emitted identifiers are passed through unchanged to app-server. Stored
profiles containing a removed identifier remain launchable because model
selection is a pass-through value; they are simply no longer advertised.

## Technical Approach

1. Reuse the existing `max` and `ultra` reasoning-option sets.
2. Replace the `model_selector.models` vector with the exact visible catalog
   returned by `model/list`, using human-readable labels consistent with the
   existing UI style.
3. Update `discovered_models_match_current_chatgpt_catalog` with the exact
   ordered entries and effort sets.
4. Avoid touching wire-preservation tests that intentionally use older model
   strings unless they incorrectly claim to test the current picker catalog.

## Compatibility and Risks

- A wrong reasoning level would create a selection that fails only when a turn
  starts. The regression test must therefore pin the verified level set.
- Adding GPT-6.2 from the public API guide would be premature: the pinned
  ChatGPT Codex app-server catalog does not expose those identifiers. The
  deployed binary is the compatibility authority for this picker refresh.
- Removed advertised entries can remain in saved profiles and continue to pass
  through to Codex; this change does not migrate stored configuration.

## Verification

- Run the focused `executors` Codex catalog unit test.
- Run formatting and the repository-required checks proportionate to this
  Rust-only change.
- Review the diff independently and resolve all significant findings.
- Confirm the homelab repository and all non-Vibe-Kanban services are unchanged.

## Source Basis

Official OpenAI API documentation identifies GPT-6.2 Astra and Sol as the
latest API family, but the deployed ChatGPT Codex integration has a distinct
catalog boundary. A read-only `model/list` query against the pinned Codex
0.155.1 app-server establishes the six selectable identifiers, visibility,
ordering, and supported reasoning efforts used by this change.
