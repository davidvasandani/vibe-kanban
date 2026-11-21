# Implementation Plan: Upgrade Codex Models

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The change is in the Rust `executors` crate, in
`crates/executors/src/executors/codex.rs`. Codex implements
`StandardCodingAgentExecutor::discover_options`, which returns an
`ExecutorDiscoveredOptions` patch consumed by the shared model-picker UI. The
catalog uses `ModelInfo` records and shared `ReasoningOption` values. No storage,
API schema, frontend, generated type, or deployment change is required.

The compatibility source is the deployed Nix-provided `codex-cli 0.155.1`.
Its initialized app-server `model/list` response was queried read-only with
`includeHidden: true`, allowing the plan to distinguish the six visible models
from two hidden internal entries and to record exact supported reasoning levels.

## Architecture & Approach

1. Keep the existing discovery architecture and edit only the ordered
   `ModelSelectorConfig.models` vector.
2. Keep `max_reasoning_options` for Luna models and
   `ultra_reasoning_options` for Astra, Sol, GPT-5.6 Sol, and GPT-5.6 Terra.
   Remove the now-unused `xhigh_reasoning_options` builder after GPT-5.5 and
   GPT-5.3 Codex Spark leave the catalog.
3. Preserve `model_aliases`, permission choices, slash commands, launch
   configuration, and all continuation/wire behavior.
4. Update `discovered_models_match_current_chatgpt_catalog` so one assertion
   pins the full public contract: order, IDs, labels, and effort IDs.
5. Leave model strings in continuation and request serialization tests alone;
   those prove pass-through preservation, not picker membership.

## Data Model

No persistent data model changes. Existing stored `ExecutorProfile` model
values remain untouched and continue through the existing selection resolver.

## Contracts

See `./contracts/codex-model-catalog.md` for the exact discovery contract.

## Research Notes

See `./research.md`. No dependency is added or upgraded.

## Constitution Check

- Principle II: the complete discovery contract is asserted by a focused unit
  test.
- Principles III and VI: the smallest existing catalog surface is updated; no
  new registry or frontend copy is introduced.
- Principle IX: identifiers, visibility, and capability levels come from the
  pinned executable that actually runs, not only public API documentation.
- Principle L: discovery removal does not delete configuration, and explicit
  saved selections remain representable through existing picker behavior.
- Constraint on generated files: no generated contract changes are expected;
  `generate-types:check` verifies that assumption.

No constitution deviation or open question remains.

## Verification Plan

1. Install locked workspace dependencies if they are absent, per `AGENTS.md`.
2. Run Rust formatting through the repository format command.
3. Run the focused Codex discovery catalog test.
4. Run the `executors` crate checks/tests proportionate to the one-file change.
5. Run `pnpm run generate-types:check` and `git diff --check`.
6. Inspect status and diff in both repositories to prove scope containment.
7. Run independent Codex review and repeat verification after any corrections.

## Risks & Dependencies

- The ChatGPT catalog can differ from the public API latest-model family. This
  plan deliberately binds to the deployed app-server.
- Catalog data may change server-side for the same CLI. The captured exact test
  makes drift deliberate; a future refresh should rerun `model/list`.
- The full repository check may expose unrelated environmental failures. Focused
  test evidence and any unrelated failures will be reported separately rather
  than hidden.
