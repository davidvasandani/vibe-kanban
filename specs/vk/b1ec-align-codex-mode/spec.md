# Feature Specification: Align Codex Model Catalog with the Current Selector

**Feature dir**: `specs/vk/b1ec-align-codex-mode/`
**Status**: Draft

## Summary

Vibe Kanban's Codex model dropdown should offer the same models, in the same
order, as the model selector Codex shows its users today. PR #404 derived the
catalog from the older deployed CLI's model list. That list has no GPT-6.2 or
GPT-6.1 entries, so the dropdown is already stale. This feature corrects the
catalog against the current selector and locks it in with an exact regression
test.

## User Stories

- As a Vibe Kanban user, I want the newest Codex models (GPT-6.2 Astra,
  GPT-6.2 Sol, GPT-6.1 Sol, GPT-6.1 Terra) in the dropdown so I can use them
  from Vibe Kanban as I can in Codex.
- As a user moving between Codex and Vibe Kanban, I want the same model order
  in both so I can find a model without relearning the list.
- As a user choosing a reasoning level, I want only levels the chosen model
  supports, so a turn doesn't fail after it starts.
- As a user with a saved explicit model, I want the refresh to leave my saved
  choice intact.

## Functional Requirements

- FR-1: The Codex dropdown advertises exactly these models, in this order:
  GPT-6.2 Astra, GPT-6.2 Sol, GPT-6.1 Sol, GPT-6.1 Terra, GPT-6 Astra,
  GPT-6 Sol, GPT-6 Luna, GPT-5.6 Sol, GPT-5.6 Terra, GPT-5.6 Luna.
- FR-2: Each entry uses the vendor model identifier Codex reports for it:
  `gpt-6.2-astra`, `gpt-6.2-sol`, `gpt-6.1-sol`, `gpt-6.1-terra`,
  `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-5.6-sol`, `gpt-5.6-terra`,
  `gpt-5.6-luna`.
- FR-3: Each entry offers exactly the reasoning efforts Codex reports for that
  model. GPT-6 Luna and GPT-5.6 Luna offer low, medium, high, xhigh and max.
  Every other entry also offers ultra.
- FR-4: Models Codex marks hidden or internal (`gpt-reserve`,
  `codex-auto-review`) are not advertised.
- FR-5: A model is kept only if it appears in the current selector. No current
  entry is dropped.
- FR-6: An automated test fails on any missing, extra, reordered, relabelled or
  wrongly configured entry.
- FR-7: Saved explicit selections and launch behaviour are unchanged. The model
  string is passed through as before.
- FR-8: Project knowledge records that the current selector is the target, and
  that Codex gates model availability by CLI version.

## Out of Scope

- Bumping the Codex CLI: the Renovate-managed npx pin, or the homelab
  `pkgs.codex` that the deployment actually runs.
- Homelab deployment configuration and other services.
- Fetching the model list dynamically at runtime.
- Changes to the default choice, permissions, aliases or slash commands.

## Acceptance Criteria

- [ ] `discover_options` for Codex returns the ten entries of FR-1/FR-2, with
      the efforts of FR-3, in order.
- [ ] The exact catalog test passes, and fails when an entry is removed, added,
      reordered or has its efforts changed.
- [ ] `gpt-reserve` and `codex-auto-review` are absent.
- [ ] `cargo test -p executors`, `pnpm run generate-types:check` and
      `pnpm run format` pass.
- [ ] The independent Codex review reports no significant findings.
- [ ] `wiki/executor-model-catalogs.md` reflects the new compatibility rule.

## Open Questions

Resolved in `clarifications.md`:

- Ordering: use the supplied selector order (version descending, then
  Astra > Sol > Terra > Luna). The task names this order and makes the
  user-facing selector authoritative.

- Ship gate: resolved 2026-11-25. The operator chose "upgrade first". Homelab
  davidvasandani/homelab#1815 moved every Codex pin to 0.171.0 and was merged
  and deployed, and the deployed runtime runs the new models
  (`clarifications.md` Q5).
