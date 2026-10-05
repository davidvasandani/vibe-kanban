# Prior knowledge: Disable unused agents (`vk/2e22-disable-agent`)

Sources: the VK wiki (`wiki/`, read-only recall), `docs/knowledge-base/`, and
the homelab knowledge bases. No page covers disabling or hiding whole agents.
The closest precedent is per-agent model hiding (`disabled_models`).

## Per-agent preferences on `ExecutorProfile` (`wiki/model-picker-preferences.md`)

There is a checklist for adding a per-agent preference field next to the
flattened variant map:

- **Serde:** `default` plus `skip_serializing_if`, so old files load and
  unchanged files stay byte-clean. Declared fields are consumed before
  `#[serde(flatten)]`, so the key is never parsed as a variant.
- **Merge:** `merge_with_defaults` and `compute_overrides` must both carry
  the field. Otherwise a preference-only change never reaches
  `profiles.json`.
- **Struct literals:** `crates/executors/src/env.rs`,
  `crates/local-deployment/src/container.rs` and
  `crates/worker/src/execution.rs`. The worker copies preferences from the
  source profile.
- **Frontend:** add the key to `RESERVED_KEYS` in
  `web-core/src/shared/lib/executor.ts`, or it renders as a configuration in
  Settings.
- **Writers spread the profile:** `updateRecentModelEntries` spreads the
  executor profile when the model picker records a recent model and saves
  the *whole* profiles file. A new field survives only because of that
  spread.

## Hiding without breaking the trigger

From the same page: filter only the list handed to the menu, always keep
the current selection visible, and never allow the last item to be disabled
(`toggleDisabledModel` refuses to disable the last model).

## Stale copies overwrite saves

Settings saves profiles via `useSettingsMachineClient()` (it can target
another host). After a save, invalidate the whole `['user-system']` prefix.
Otherwise a route-scoped chat copy re-saves stale profiles on its next
recent-model write and silently reverts the change. `AgentsSettingsSection`
already does this in `refreshProfileViews()`.

## Built-in executors cannot be deleted

`compute_overrides` returns `CannotDeleteExecutor` when a built-in executor
is missing. "Disable" must therefore be a flag, not a removal.

## i18n gate (`docs/knowledge-base/locale-key-consistency.md`, `wiki/issue-workspace-advisory.md`)

Every new `settings.agents.*` key must exist in all seven locales
(`en, es, fr, ja, ko, zh-Hans, zh-Hant`). An English fallback alone fails
`scripts/check-i18n.sh`, and the gate also checks interpolation identifiers.

## Lint and test notes (`wiki/frontend-linting.md`)

`pnpm run lint` covers local-web, web-core, remote-web and ui. `web-core`
test files are linted through a separate tsconfig. Vitest tests sit next to
the code (for example `shared/lib/disabledModels.test.ts`).

## Settings drawer (`wiki/settings-drawer.md`)

A section's dirty flag is cleared on unmount. Edits should go through the
existing dirty and save-bar flow (`useSettingsDirty`, `SettingsSaveBar`) so
the drawer's unsaved-changes guard covers them.
