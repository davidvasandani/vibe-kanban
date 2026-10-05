# Data model: Disable unused agents

## `ExecutorProfile` (per agent, per host): `crates/executors/src/profile.rs`

| Field | Type | Default | Serialized when | Meaning |
|---|---|---|---|---|
| `recently_used_models` | `Option<ExecutorRecentModels>` | `None` | `Some` | existing |
| `disabled_models` | `Vec<String>` | `[]` | non-empty | existing |
| **`disabled`** | **`bool`** | **`false`** | **`true`** | **new.** Hidden from agent pickers. Still configurable in Settings. |
| `configurations` (flattened) | `HashMap<String, CodingAgent>` | n/a | always | existing variants |

The record lives in `ExecutorConfigs.executors[BaseCodingAgent]`. Defaults
come from `default_profiles.json` (never disabled). User overrides live in
`profiles.json`.

### Merge and override rules
- merge: `override.disabled == true` sets the merged profile's `disabled` to
  `true`.
- overrides: write `disabled` when `current.disabled != default.disabled`. A
  profile whose only difference is `disabled` is still written.
- Re-enabling yields `false`, which equals the default, so nothing is
  written, and the agent drops out of the overrides if nothing else differs.

## Derived frontend state
- `isAgentDisabled(profiles, agent)` is `profiles[agent]?.disabled === true`.
- Disable blocker:
  - `'default'` if `agent === config.executor_profile.executor`
  - else `'last'` if no other agent is enabled
  - else `null`
