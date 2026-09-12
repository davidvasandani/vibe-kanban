# Technical Specification: Claude Fable 5.1 and Claude Code Refresh

**Task:** `vk/1f95-update-claude-mo`

**Service:** Vibe Kanban

**Date:** 2026-09-11

## Objective

Update Vibe Kanban's Claude executor so users can select the newly released
Fable 5.1 model and executions use the newest supported Claude Code dependency.
Keep the service deployment compatible with the updated executor and preserve
the safety controls that Vibe Kanban applies to Claude Code.

## Scope

- Refresh the pinned `@anthropic-ai/claude-code` package used by the Vibe
  Kanban Claude executor to the latest published version verified during this
  task.
- Add the canonical Fable 5.1 model identifier and display label to the Claude
  model selector, using vendor/CLI evidence for the exact identifier.
- Re-verify version-sensitive Claude Code behavior, especially model aliases,
  supported effort values, protocol compatibility, and the native wire names
  used by Vibe Kanban's denied background/scheduling tool controls.
- Update focused tests and documentation/comments that deliberately pin the
  Claude Code version or model catalog.
- Inspect `homelab/modules/vibe-kanban-rebuild.nix`, the governing deployment
  module, and change it only if its Claude Code dependency or compatibility
  contract must move with the Vibe Kanban source change.
- Record reusable findings in the Vibe Kanban knowledge base and refresh its
  index.

## Out of Scope

- Changes to services other than Vibe Kanban.
- Unrelated executor, UI, or infrastructure changes.
- Replacing Vibe Kanban's process-lifecycle safety policy or enabling Claude
  Code background jobs inside a turn.
- Deploying or switching a NixOS host configuration.

## Functional Requirements

1. The default Claude model catalog MUST expose Fable 5.1 with the exact model
   argument accepted by the refreshed Claude Code CLI.
2. Existing Claude model choices and the default model MUST remain available
   unless authoritative release documentation says they were removed.
3. Fable 5.1 MUST expose every reasoning-effort choice supported by the Claude
   executor's current contract.
4. Fable 5.1 usage MUST be measured against its native 1M-token context window
   before the end-of-turn usage report is available.
5. The standard Claude executor command MUST pin the latest verified
   `@anthropic-ai/claude-code` release rather than use an unbounded tag.
6. All version-coupled comments and tests MUST agree with that pin.
7. The update MUST preserve the explicit denial of unsupported wake-up and
   in-turn background-poller behavior.
8. Focused automated tests MUST cover the new model entry, reasoning options,
   context window, exact dependency pin, and version-sensitive safety
   assumptions.
9. The governing Nix module MUST continue to provide the runtime prerequisites
   required by the refreshed executor; no other homelab service may be changed.

## Verification

- Confirm current model and CLI facts against authoritative Anthropic sources
  and the published package/native binary.
- Install repository dependencies with the frozen lockfile before project
  verification.
- Run focused Claude executor tests, formatting, and the relevant repository
  checks.
- If the Nix module changes, run `nixfmt --check`, `nix-instantiate --parse`,
  and the narrow Vibe Kanban configuration evaluation available in the
  homelab repository.
- Run an independent Codex diff review and resolve all significant findings.

## Acceptance Criteria

- Fable 5.1 is selectable in Vibe Kanban's Claude model selector and is passed
  unchanged to Claude Code.
- Fable 5.1 usage is calculated against a 1M-token context window.
- Vibe Kanban launches the latest Claude Code version verified on 2026-09-11.
- Alias/release-note and native-tool-name checks are documented and reflected
  in tests or code comments where version coupling exists.
- Relevant automated checks pass.
- The task's specification, plan, SpecKit artifacts, review evidence, and
  reusable knowledge are committed; a pull request is opened against the base
  branch and merged.

## Risks

- A Claude Code bump can silently change `opus`, `sonnet`, `haiku`, or `fable`
  alias resolution even when compilation and tests pass.
- Native tool names can change between Claude Code releases, weakening Vibe
  Kanban's deny rules if they are not inspected directly.
- A guessed Fable 5.1 identifier could render a visible but unusable selector
  choice; authoritative evidence is required before implementation.
