# Implementation Plan: Align Codex Model Catalog with the Current Selector

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- Rust executor crate `crates/executors`. The catalog is static data built in
  `Codex::discover_options` (`crates/executors/src/executors/codex.rs`) and
  emitted as one JSON patch whose `model_selector.models` the shared frontend
  renders. No frontend copy of the list exists.
- `ReasoningOption::from_names` and `ReasoningEffort` (with `Max`/`Ultra`)
  already exist. No new type, field, dependency or generated TS change.
- Runtime launches pass the model string unchanged to the Codex app-server.
  The deployed binary comes from `VIBE_CODEX_COMMAND` (homelab `pkgs.codex`,
  currently 0.155.1).

## Architecture & Approach

1. **Contract test first (FR-6).** Replace the expected vector in
   `tests::discovered_models_match_current_chatgpt_catalog` (`codex.rs`,
   around line 40) with the ten-entry table in
   `contracts/codex-model-catalog.md`. The existing assertion already compares
   ID, label and reasoning option IDs as an ordered vector, which catches
   missing, extra, reordered or misconfigured entries.
2. **Catalog (FR-1–FR-5).** In `discover_options` (around line 1072), prepend
   four `ModelInfo` entries (`gpt-6.2-astra`, `gpt-6.2-sol`, `gpt-6.1-sol`,
   `gpt-6.1-terra`) using `ultra_reasoning_options.clone()`. Keep the six
   existing entries and their efforts. Hidden models are not added (FR-4).
3. **Compatibility (FR-7).** No change to profile storage, `model_aliases`,
   defaults or the launch path. Saved values remain plain strings.
4. **Knowledge (FR-8).** Update `wiki/executor-model-catalogs.md` and
   `wiki/INDEX.md`: the selector is the target, the backend gates by client
   version, and the minimum CLI version for the advertised set must be
   deployed before release.

## Data Model

Not applicable. No persisted or typed shape changes.

## Contracts

See `./contracts/codex-model-catalog.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- **II Test the contract:** the exact ordered test is updated first and must
  fail before the catalog edit. ✔
- **III/VI Small, reuse existing source:** one existing vector edited, with no
  new plumbing or dynamic fetch. ✔
- **IX Defensive vendor contracts:** IDs and efforts come from the vendor's
  `model/list`. The client-version gate is recorded (b1ec constitution review)
  and enforced as a ship gate. ✔ (with gate)
- **L Never strand a selection:** no entry is removed, and saved values pass
  through. ✔
- Scope: homelab is not modified. The runtime upgrade is an operator decision
  (spec open question).

## Risks & Dependencies

- **Deployed CLI rejects the new models (high).** Codex 0.155.1 returns
  `model_not_found` for all four new IDs. Mitigation: hold the merge until the
  operator chooses to upgrade homelab `pkgs.codex` to a version that serves
  them (0.171.0 verified) or to accept the gap.
- **Order divergence.** The Codex TUI sorts by backend priority, which differs
  from the specified selector order. It is documented, and a future refresh
  must re-check it.
- **Catalog churn.** The backend can change the list at any time. The exact
  test makes drift a conscious update.
