# Disabling agents: one flag, one resolver, every picker

Users can hide coding agents they don't use (Settings → Agents → Visibility).
This page records the decisions behind that and the places a new agent
picker has to plug into.

## Storage: `ExecutorProfile.disabled` in `profiles.json`

- The flag is `disabled: bool` on `ExecutorProfile`, next to
  `disabled_models`. It uses serde `default` with
  `skip_serializing_if = "std::ops::Not::not"`, so `false` is never written.
  The [[model-picker-preferences]] checklist applies: merge/override,
  struct literals, `RESERVED_KEYS`.
- It is per host, because Settings → Agents edits one host's profiles. It
  saves through the Agents save bar like every other agent edit.
- Rejected alternatives:
  - `disabled_agents` on the user `Config`. That would be a second store with
    an immediate-save path and per-user rather than per-host scope.
  - Deleting the executor. `compute_overrides` rejects that with
    `CannotDeleteExecutor`, and it would lose the agent's configurations.
- It is a **display filter, not access control** (constitution L). Nothing
  on the backend refuses to run a disabled agent, so existing sessions,
  pipelines, remediation and MCP `start_workspace` keep working. The only
  backend consumer is `recommendation_candidates()`, so onboarding never
  recommends a disabled agent.

## One frontend resolver

`web-core/src/shared/lib/disabledAgents.ts` is the only place that decides
what "disabled" means:
- `isAgentDisabled`
- `filterEnabledAgents(agents, profiles, keep)`
- `setAgentDisabled`
- `getDisableAgentBlocker`

The rules:
- **Keep the current value visible.** Pass the picker's current value as
  `keep`, so a trigger never names an agent missing from its own menu.
- **No dead end.** If no agent is enabled (only possible by hand-editing the
  file), offer every agent, not just the kept one.
- **Implicit fallbacks skip disabled agents. Explicit choices don't.** In
  `resolveEffectiveExecutor` (`useExecutorConfig.ts`), last used and the
  config default skip disabled agents. The user's selection and the scratch
  draft are honoured.
- **Guards read local state.** Settings checks the *unsaved* profiles. The
  default agent and the last enabled agent are locked, and "Make Default"
  is unavailable for an agent whose disable is still pending.

## Picker inventory

Every UI that lists agents enumerates `Object.keys(profiles)`. As of
`vk/2e22` they are:

| Picker | Location |
|---|---|
| create-workspace and new-session chat boxes | `useExecutorConfig` |
| Start review and Resolve conflicts | `AgentSelector` |
| default agent and remediation agent | `GeneralSettingsSection` |
| onboarding | `LandingPage` |

A new picker must go through `filterEnabledAgents`. To find any that were
missed, search for `Object.keys(profiles` and `Object.values(BaseCodingAgent)`.

## i18n trap

`settings.agents.availability.*` already exists and holds the strings for
the agent *availability check* (installed / login detected), even though
nothing renders them right now. The new card uses
`settings.agents.visibility.*`. Rewriting a locale file with a script that
assigns an existing key silently replaces the whole subtree. Check that a
key is absent before adding it.

## Contributed by

- `vk/2e22-disable-agent`
