# Implementation plan: model menu (vk/6823-model-menu)

See `SPEC.md`. Steps in dependency order; each lands with its tests.

## 1. Claude catalog (R2, R3) — `crates/executors/src/executors/claude.rs`
- Replace the model list in `default_discovered_options` with the five
  versioned entries (`claude-opus-5-5`, `claude-opus-5`, `claude-sonnet-5`,
  `claude-fable-5-1`, `claude-haiku-4-5`).
- `default_model: Some("claude-opus-5-5")`.
- Leave `context_window_for_model` alias arms (saved configs still use them).
- Tests: replace the three per-model tests with one exact ordered catalog test
  (IDs, labels, effort IDs) + default assertion; keep context-window tests.

## 2. `disabled_models` on profiles (R4 backend) — `crates/executors/src/profile.rs`
- `ExecutorProfile.disabled_models: Vec<String>` with
  `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, declared before
  the flattened `configurations`.
- `merge_with_defaults`: override wins when non-empty.
- `compute_overrides`: carry when it differs from defaults; include profile
  when non-empty.
- Update struct literals: `env.rs`, `local-deployment/src/container.rs`,
  `worker/src/execution.rs` (propagate from source profile), `profile.rs`.
- Unit test: round-trip JSON with `disabled_models`, merge + overrides.
- `pnpm run generate-types` → `shared/types.ts`.

## 3. Frontend helpers — `packages/web-core/src/shared/lib/`
- `executor.ts`: add `disabled_models` to `RESERVED_KEYS`.
- New `disabledModels.ts`: `getDisabledModelEntries`, `isModelDisabled`,
  `toggleDisabledModel` (never disables the last enabled model),
  `updateDisabledModels(profiles, executor, entries)`,
  `filterDisabledModels(config, disabled, keepKey)`.
- `disabledModels.test.ts` (Vitest).

## 4. Picker (R1, R4) — `packages/ui/.../ModelSelectorPopover.tsx`, `ModelSelectorContainer.tsx`
- Popover: new `autoFocusSearch?: boolean` (default `true`) forwarded as
  `autoFocus` to `DropdownMenuSearchInput`.
- Container: `autoFocusSearch={!(useIsMobile() || useIsRealMobile())}`.
- Container: pass a config filtered by `disabled_models`, keeping the selected
  model visible.

## 5. Settings UI (R4) — `AgentsSettingsSection.tsx`
- New `AgentModelsCard` below the config form for the selected executor:
  uses `useModelSelectorConfig(executor)`, renders checkbox rows (label + id),
  toggles `disabled_models` in `localParsedProfiles` and marks dirty.
- `handleSave`: save `localParsedProfiles` whenever dirty (not only when the
  selected variant's form data exists).
- i18n strings in `settings` namespace (en + existing locales fallback).

## 6. Verify
- `SQLX_OFFLINE=true cargo test -p executors`, `pnpm run check`,
  `pnpm run lint`, web-core Vitest, `pnpm run format`.
