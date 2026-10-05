# SPEC: Disable unused coding agents

Task: `vk/2e22-disable-agent`. Feature spec, plan and tasks:
`specs/vk/2e22-disable-agent/`.

## Problem

Settings → Agents lists every built-in executor (Claude Code, Codex, Gemini,
Amp, Opencode, Cursor, Qwen Code, Copilot, Droid, Grok). Every agent picker
shows all of them too: the create-workspace and new-session chat boxes, the
General default-agent dropdown, the auto error remediation agent, the review
and conflict dialogs, and onboarding. Most hosts use only one or two agents,
so the rest clutter every picker. Built-in executors cannot be deleted
(`ProfileError::CannotDeleteExecutor`), so there is no way to remove them
today.

## Goal

Let the user turn individual agents off in Settings → Agents. A disabled agent
stays configurable in Settings but disappears from every agent picker. It can
be turned back on at any time.

## Design

### Storage: a per-agent flag on `ExecutorProfile` (`profiles.json`)

Add `disabled: bool` to `ExecutorProfile` (`crates/executors/src/profile.rs`),
next to `recently_used_models` and `disabled_models`:

```rust
/// Hidden from agent pickers. The agent stays configurable in Settings.
#[serde(default, skip_serializing_if = "std::ops::Not::not")]
pub disabled: bool,
```

- It is stored per host, alongside the rest of that host's agent
  configuration, and is edited and saved through the same Save bar the Agents
  page already has.
- It already reaches every picker: `GET /api/info` returns
  `executors` (flattened `ExecutorConfigs`), which `useUserSystem().profiles`
  exposes.
- Serde reads declared fields before `#[serde(flatten)]`, so `disabled` is
  never parsed as a variant. `skip_serializing_if` keeps unchanged files
  identical.
- `merge_with_defaults` copies `disabled = true` from overrides.
  `compute_overrides` writes the field when it differs from the default
  (built-in defaults are always `false`), and a profile whose only change is
  `disabled` is still written to the overrides file.
- Struct literals in `env.rs`, `local-deployment/container.rs` and
  `worker/execution.rs` gain the field. The worker copies it from the source
  profile, as it does with the other preferences.
- `get_recommended_executor_profile` skips disabled agents, so onboarding
  never recommends one.

Rejected alternative: `disabled_agents: Vec<BaseCodingAgent>` on the v8 user
`Config`. Agent installation and configuration are per host and already live
in `profiles.json`. Putting the flag there means one save path, one
invalidation path (`['user-system']`), and no second source of truth.

### Frontend helpers (`web-core/src/shared/lib/disabledAgents.ts`)

- `isAgentDisabled(profiles, agent)` is the one frontend definition of
  "disabled".
- `filterEnabledAgents(agents, profiles, keep?)` keeps the order and drops
  disabled agents. Any agent in `keep` (the current selection) always stays
  visible, so a trigger never names an agent its own menu lacks. If no agent
  is enabled (only possible by hand-editing the file), it returns every
  agent.
- `setAgentDisabled(profiles, agent, disabled)` returns a new profiles map.
  It spreads the existing profile and deletes the key when the agent is
  re-enabled.
- `getDisableAgentBlocker(profiles, agent, defaultAgent)` returns
  `'default' | 'last' | null`. It reads the Settings page's *local*
  (unsaved) profiles, and re-enabling is never blocked.
- `disabled` is added to `RESERVED_KEYS` in `shared/lib/executor.ts` so it
  never shows up as a configuration.

### Settings → Agents

- Agents column: a disabled agent row is dimmed and shows a neutral
  **Disabled** badge. The order is unchanged.
- A new **Visibility** card for the selected agent, above the Models card,
  has a checkbox labelled **Show in agent pickers**:
  - Unchecking it marks the page dirty. Saving writes `disabled: true`.
  - It is locked when the agent is the current default ("The default agent
    can't be disabled. Make another agent the default first.").
  - It is locked when the agent is the last enabled one ("At least one agent
    must stay enabled.").
- **Make Default** is unavailable for a disabled agent's configurations,
  including a disable that hasn't been saved yet, so the default agent is
  never a disabled one.

### Pickers

All pickers filter with `filterEnabledAgents`. Each keeps its current value
visible:

| Picker | Kept visible |
|---|---|
| `useExecutorConfig` (create-workspace and new-session chat boxes) | the effective executor |
| `AgentSelector` (Start review, Resolve conflicts) | `selectedExecutorProfile.executor` |
| General → default agent dropdown | the draft's default executor |
| General → auto error remediation agent | the configured remediation executor |
| Onboarding `LandingPage` | the selected agent |

In `useExecutorConfig` (`resolveEffectiveExecutor`), the fallback chain skips
`lastUsedConfig` and the config default when their agent is disabled. In
that case the first enabled agent wins. An explicit user selection and the scratch draft are
still honoured.

## Non-goals

- The backend does not refuse to run a disabled agent. Existing sessions,
  follow-ups, MCP `start_workspace` calls, pipelines and remediation keep
  working. Disabling is about picker clutter, not access control.
- The agent list is not reordered.
- Per-configuration (variant) disabling is out of scope.

## Acceptance criteria

1. In Settings → Agents, unchecking "Show in agent pickers" for Qwen Code and
   saving writes `"disabled": true` under `QWEN_CODE` in `profiles.json`. The
   row shows a Disabled badge.
2. Qwen Code no longer appears in the create-workspace agent picker, the
   new-session picker, General's default-agent dropdown, the auto remediation
   agent select, or the review and conflict dialogs.
3. Re-checking the box and saving removes the key, and the agent reappears
   everywhere.
4. The default agent's checkbox is locked with an explanation, and so is the
   last enabled agent's.
5. Existing `profiles.json` files without the key load unchanged, and saving
   without touching the flag writes no `disabled` key.
6. `cargo test -p executors` covers the serde round-trip, omission when false,
   and survival through override and merge. Vitest covers the helper module.
