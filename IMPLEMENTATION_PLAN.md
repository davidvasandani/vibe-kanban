# Implementation plan: Disable unused agents

Task `vk/2e22-disable-agent`. Spec: `SPEC.md`. Prior knowledge:
`PRIOR_KNOWLEDGE.md`. SpecKit: `specs/vk/2e22-disable-agent/`.

## A. Backend (Rust)

1. `crates/executors/src/profile.rs`
   - Add `disabled: bool` to `ExecutorProfile` with
     `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` and a
     doc comment.
   - `merge_with_defaults`: `if override_profile.disabled { default_profile.disabled = true; }`.
   - `compute_overrides`: initialise `disabled: false`. Set it from
     `current_profile.disabled` when that differs from the default. Include
     the profile in the overrides when `disabled` is set.
   - `get_recommended_executor_profile`: skip profiles with
     `disabled == true`.
   - Tests:
     - serde round-trip (`disabled` is not a variant)
     - `false` is omitted on serialize
     - survives `compute_overrides` and `merge_with_defaults`
     - re-enabling produces no override
     - the recommended profile ignores disabled agents (use a pure helper so
       the test does not depend on installed CLIs)
2. Struct literals: `crates/executors/src/env.rs` and
   `crates/local-deployment/src/container.rs` get `disabled: false`.
   `crates/worker/src/execution.rs` copies `disabled` from the source
   profile.
3. `pnpm run generate-types` adds `disabled?: boolean` to `ExecutorProfile`
   in `shared/types.ts`.

## B. Frontend helpers

4. `packages/web-core/src/shared/lib/executor.ts`: add `'disabled'` to
   `RESERVED_KEYS`.
5. New `packages/web-core/src/shared/lib/disabledAgents.ts`:
   - `isAgentDisabled(profiles, agent)`
   - `filterEnabledAgents(agents, profiles, keep?)`
   - `setAgentDisabled(profiles, agent, disabled)`
   - `getDisableAgentBlocker(profiles, agent, defaultAgent)` returns
     `null | 'default' | 'last'`
6. `disabledAgents.test.ts` (Vitest) next to it.

## C. Settings → Agents

7. `AgentsSettingsSection.tsx`:
   - Agents column: dim disabled rows and add a neutral `Disabled` badge.
   - New `AgentVisibilityCard` above `AgentModelsCard`, with a
     `SettingsCheckbox` "Show in agent pickers". It is locked with an
     explanation when the blocker is `default` or `last`. `onChange` →
     `markDirty(setAgentDisabled(...))`.
   - `ConfigActionsDropdown`: add an `agentDisabled` prop that disables
     "Make Default".
8. i18n: add `settings.agents.editor.disabled` and
   `settings.agents.visibility.{title,description,label,defaultLocked,lastLocked}`
   to all seven locales.

## D. Pickers

9. `useExecutorConfig.ts` → `useEffectiveExecutor`:
   - Skip disabled agents for `lastUsedConfig` and the config default.
   - Fall back to the first enabled option.
   - `options = filterEnabledAgents(all, profiles, [effective])`.
10. `AgentSelector.tsx`: filter with `keep = selectedAgent`.
11. `GeneralSettingsSection.tsx`:
    - Default-agent dropdown: filter with `keep = draft executor`.
    - Remediation executors: filter with
      `keep = draft.auto_error_remediation.executor`.
12. `LandingPage.tsx`: filter with `keep = selectedAgent`.

## E. Verify

13. Run `cargo test -p executors`, `cargo check --workspace`,
    `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`,
    web-core Vitest (`disabledAgents`, settings tests),
    `scripts/check-i18n.sh` and `pnpm run format`.
14. If a dev build is feasible, check in the app UI that toggling hides the
    agent from the create-workspace picker.

## F. Ship

15. Codex review, then fix findings, then write the wiki page. Update
    `wiki/model-picker-preferences.md` (the `disabled` field) and add a
    `wiki/` entry for agent disabling. Then open the PR, wait for CI, and
    merge.
