# Feature Specification: Upgrade Codex Models

**Feature dir**: `specs/vk/3ffa-upgrade-codex-mo/`
**Status**: Draft

## Summary

Refresh Vibe Kanban's Codex model choices so users see the models that the
deployed Codex integration actually makes available, with only valid reasoning
levels. The current picker is stale: it omits two GPT-6 choices, understates
GPT-6 Astra's capability, and advertises two choices no longer present in the
current Codex menu.

## User Stories

- As a Vibe Kanban user, I want the Codex picker to show all current GPT-6
  choices so I can choose the cost, speed, and capability tier suited to my
  work.
- As a user selecting a reasoning level, I want every offered level to be
  supported by the chosen model so a turn does not fail after it starts.
- As a user with an older saved model selection, I want the catalog refresh not
  to rewrite or delete my explicit configuration.

## Functional Requirements

- FR-1: The Codex picker must advertise these explicit models in this order:
  GPT-6 Astra, GPT-6 Sol, GPT-6 Luna, GPT-5.6 Sol, GPT-5.6 Terra, and GPT-5.6
  Luna.
- FR-2: The advertised identifiers must be `gpt-6-astra`, `gpt-6-sol`,
  `gpt-6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra`, and `gpt-5.6-luna`,
  respectively.
- FR-3: GPT-6 Astra, GPT-6 Sol, GPT-5.6 Sol, and GPT-5.6 Terra must offer
  `low`, `medium`, `high`, `xhigh`, `max`, and `ultra` reasoning.
- FR-4: GPT-6 Luna and GPT-5.6 Luna must offer `low`, `medium`, `high`,
  `xhigh`, and `max` reasoning, but not `ultra`.
- FR-5: GPT-5.5, GPT-5.3 Codex Spark, and hidden internal Codex models must not
  appear as advertised choices.
- FR-6: The catalog refresh must not change the separate Default choice,
  permissions, saved executor profiles, launch behavior, session behavior, or
  model values already stored by users.
- FR-7: The complete ordered picker contract must have automated regression
  coverage for identifiers, labels, and reasoning levels.

## Out of Scope

- Upgrading or repinning the Codex CLI.
- Adding GPT-6.2 API models before they appear in the deployed ChatGPT Codex
  catalog.
- Changing any non-Codex executor, frontend, authentication flow, prompt,
  service tier, session lifecycle, or homelab service.
- Migrating or deleting saved selections that refer to older model IDs.

## Acceptance Criteria

- [ ] Codex discovery returns exactly the six models in FR-1 and FR-2, in that
      order.
- [ ] Each model returns exactly the reasoning levels in FR-3 or FR-4.
- [ ] No hidden or superseded model from FR-5 is advertised.
- [ ] The focused catalog regression test passes.
- [ ] Formatting and relevant executor checks pass without unrelated generated
      changes.
- [ ] The homelab repository and all services other than Vibe Kanban remain
      unchanged.

## Open Questions

- None. The deployed Codex app-server provides exact identifiers, visibility,
  order, and reasoning capabilities for this refresh.
