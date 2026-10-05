# Research: Disable unused agents

## Decision: a flag on `ExecutorProfile` (`profiles.json`)
- **Chosen:** `disabled: bool` on `ExecutorProfile`, next to
  `recently_used_models` and `disabled_models`.
- **Why:** Settings → Agents already loads, edits and saves exactly this
  file for the selected host. `/api/info` already ships `executors` to every
  picker. The `disabled_models` precedent shows the full checklist (serde,
  merge, struct literals, `RESERVED_KEYS`).
- **Rejected: `disabled_agents: Vec<BaseCodingAgent>` on v8 `Config`.** That
  would be a second store with a second save path (`PUT /api/config`, saved
  immediately, not via the Agents save bar). Config is per user, while
  agent installs are per host. Rejected by constitution L.
- **Rejected: removing the executor from the overrides.** `compute_overrides`
  forbids deleting built-ins, and deleting would also lose the user's
  configurations.

## Decision: hide, don't block
Disabling is a display preference (spec clarifications, constitution L).
Backend execution paths are untouched, so existing workspaces, pipelines,
remediation and MCP calls keep working.

## Decision: serde shape
`#[serde(default, skip_serializing_if = "std::ops::Not::not")]`:
- A missing key reads as enabled.
- `false` is never written, so unchanged files stay identical.
- Serde consumes declared fields before `#[serde(flatten)] configurations`,
  so `disabled` is never parsed as a variant (the same mechanism that
  protects `disabled_models`, already covered by tests).
- `ts-rs` emits `disabled?: boolean`, because the field has `default`
  together with `skip_serializing_if`.

## Decision: keep the current value visible
`filterEnabledAgents(agents, profiles, keep)` mirrors `filterDisabledModels`
(`shared/lib/disabledModels.ts`), which keeps the current model. If every
agent is somehow disabled, it returns the unfiltered list instead of an
empty one.

## Decision: guard rules live in the UI
The default agent and the last enabled agent are locked in Settings. The
backend does not validate them, because disabling is not access control and
a hand-edited file must still load. Pickers stay usable through the
keep/fallback rules either way.

No new dependencies.
