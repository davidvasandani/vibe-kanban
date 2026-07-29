# Technical Specification: Reliable Parallel Sub-Agent Pipeline

## Problem

The bundled `Parallel Sub-Agents` pipeline asks the execution agent to fan a
task out to Claude, Codex, and Grok, but it leaves the launch contract
underspecified. In practice the orchestrator can disable a child agent's tools
while trying to keep the fan-out read-only, make the task prompt unavailable to
the child, wait on providers serially, or spend the bounded iteration budget
retrying an unavailable provider. The resulting synthesis is incomplete and
contains operational fallback narration instead of three useful analyses.

## Scope

Refresh the bundled pipeline definition and its regression coverage. The fix
must remain a prompt-driven pipeline; it must not add a provider orchestration
service or change the generic pipeline file format.

## Requirements

1. The fan-out stage must tell the execution agent to start Claude, Codex, and
   Grok concurrently through their available agent/CLI interfaces.
2. Every child must receive the original task prompt as its initial prompt,
   before any follow-up or shutdown instruction.
3. Child agents must retain the tools needed to read and reason about the
   workspace. Read-only analysis must be achieved through the prompt and
   permission/sandbox policy, not by disabling all tools.
4. The orchestrator must collect complete outputs and identify each output by
   provider.
5. Provider launch/authentication/unavailability failures must be reported
   concisely and must not prevent successful children from completing.
6. Later rounds must launch a fresh concurrent fan-out with the original prompt
   plus the previous synthesis, rather than treating failed attempts as rounds
   or embedding a substitute answer in place of a child response.
7. Iteration remains bounded at operator-selected `N`, defaulting to three, and
   may stop early on convergence.
8. Regression tests must assert the safety- and reliability-critical prompt
   contract so a future wording change cannot silently restore tool disabling
   or sequential retries.
9. Existing installations must receive the refreshed bundled definition via
   the existing seed-manifest upgrade mechanism without overwriting a
   user-customized pipeline.

## Acceptance Criteria

- The bundled TOML parses and retains the existing stage IDs and defaults.
- The fan-out prompt explicitly requires concurrent launches, initial delivery
  of the exact original prompt, workspace-reading capability, and forbids
  disabling all child tools.
- The iterate prompt explicitly preserves the original prompt, adds prior
  synthesis as context, uses fresh concurrent children, and does not count
  failed launches as completed rounds.
- The bundled seed manifest recognizes the previous shipped parallel pipeline
  content and upgrades it to the corrected default while preserving modified
  local copies.
- Focused pipeline service tests pass.

## Non-Goals

- Guaranteeing that all external providers are installed, authenticated, or
  available.
- Selecting provider model versions.
- Giving child agents permission to modify the workspace.
- Building a new cross-provider execution API.
