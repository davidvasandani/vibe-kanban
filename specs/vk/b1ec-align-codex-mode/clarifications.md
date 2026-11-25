# Clarifications: Align Codex Model Catalog

## Session 2026-11-25

### Q1. Which order is authoritative?

Evidence: the Codex 0.171.0 TUI `/model` picker renders backend priority
(6.2 Sol, 6.2 Astra, 6.1 Terra, 6.1 Sol, 6 Astra, 6 Sol, 6 Luna, 5.6 Sol,
5.6 Terra, 5.6 Luna). The task's selector evidence lists 6.2 Astra, 6.2 Sol,
6.1 Sol, 6.1 Terra, 6 Astra, 6 Sol, 6 Luna.

**Resolution:** use the task's order. The task states the order explicitly
("in order"), and its context names the user-facing selector the operator sees
as the authoritative target. The screenshot follows one rule throughout:
version descending, then tier Astra > Sol > Terra > Luna. Applying that rule
below the visible portion gives 5.6 Sol, 5.6 Terra, 5.6 Luna, which matches
the TUI too. The divergence from the TUI is recorded in the wiki so a future
refresh can re-check it.

### Q2. Which reasoning efforts per model?

**Resolution:** take them from 0.171.0 `model/list` `supportedReasoningEfforts`.
GPT-6 Luna and GPT-5.6 Luna: low–max. All other visible models: low–ultra.
Unchanged for the six retained entries.

### Q3. Which models are excluded?

**Resolution:** the hidden records `gpt-reserve` and `codex-auto-review`. No
visible model is excluded.

### Q4. Labels?

**Resolution:** VK's spaced style (`GPT-6.2 Astra`), consistent with the
existing catalog and the screenshot. App-server labels are inconsistent
(`GPT-6.2-Astra` vs `GPT-6.1 Terra`); the ID is the contract.

### Q5. May this merge while the deployed CLI (0.155.1) rejects the new models? — RESOLVED

Evidence: `codex exec -m gpt-6.2-sol` on 0.155.1 returns 400
`model_not_found` / `model_not_in_accessible_catalog` (re-checked 2026-11-25).
The same IDs run on 0.171.0. The runtime is `pkgs.codex` from homelab, which
this task excludes.

**Interim decision:** implement, test and review on the branch, and open the
PR. Do not merge without an explicit operator decision (merge anyway, or
upgrade the homelab Codex runtime first).

**Final decision (operator, 2026-11-25):** upgrade first, then merge. Homelab
PR davidvasandani/homelab#1815 relocked `codex-cli-nix` and bumped
`pkgs/codex` to 0.171.0, and raised the CI version floor to 0.171.0. After it
deployed, `VIBE_CODEX_COMMAND` on think1 is `codex-0.171.0`, and
`gpt-6.2-astra` and `gpt-6.2-sol` ran through it.
