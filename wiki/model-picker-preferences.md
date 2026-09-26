# Model picker preferences and touch behaviour

The chat model picker (`ModelSelectorContainer` in web-core, which drives the
`ModelSelectorPopover` in `@vibe/ui`) combines the executor's discovered
catalog with per-agent preferences stored in `profiles.json`.

## Preferences live on `ExecutorProfile`, not in the catalog

Per-agent picker state (`recently_used_models`, `disabled_models`) is stored as
named fields on `ExecutorProfile`, next to the flattened variant map. Every new
field of this kind needs four things:

- **Serde:** `default` plus `skip_serializing_if`, so older files load and
  unchanged files stay clean. Serde consumes declared fields before
  `#[serde(flatten)]`, so the field is never parsed as a variant.
- **Merge logic:** `merge_with_defaults` and `compute_overrides` must carry the
  field. Otherwise a preference-only change never reaches the overrides file.
- **Struct literals:** `env.rs`, `local-deployment` and `worker` construct
  `ExecutorProfile`. The worker copies preferences from the source profile.
- **Frontend reserved keys:** add the key to `RESERVED_KEYS` in
  `web-core/src/shared/lib/executor.ts`. Without it, the key appears as a
  variant in Settings.

Frontend writers must spread the existing profile, not rebuild it.
`updateRecentModelEntries` spreads the executor profile, so it preserves
`disabled_models`.

## Hiding models without breaking the trigger

Filter hidden models only in the config handed to the popover. Selection
resolution runs on the unfiltered catalog. Always keep the current selection
visible, so the trigger never names a model missing from its own menu. Drop
provider groups left empty, and never allow the last model to be disabled.

## Stale profile copies overwrite preferences

The picker saves the **whole** profiles file whenever it records a recent
model. Remote frontends read profiles through a route-scoped
`['user-system', 'remote-route', hostId]` query, while Settings uses
`['user-system', 'settings-machine', hostId]`. After Settings saves profiles,
invalidate the whole `['user-system']` prefix. Otherwise the chat keeps a
stale copy, and its next recent-model save reverts the new preference.

## Settings must discover on the settings host

Settings loads and saves profiles through `useSettingsMachineClient()`, which
may target a machine other than the current route. Discovery streams default
to the route host. Pass `machineClient.webSocketOptions` as `socketOptions`
through `useModelSelectorConfig` / `useJsonPatchWsStream`, so the catalog and
the saved preferences refer to the same machine. `useJsonPatchWsStream` treats
a changed socket scope as a new stream. It still calls
`openLocalApiWebSocket(endpoint)` with a single argument when no scope is
given, which existing tests assert.

## Touch: never raise the keyboard uninvited

`DropdownMenuSearchInput` autofocuses by default. On iOS, focusing an input on
open raises the keyboard over the menu. The model popover opts out with
`autoFocusSearch={false}` whenever either mobile signal is true (`useIsMobile()`
layout or `useIsRealMobile()` device; see
workspace-context-bar-responsive-visibility). The command-bar
`SelectionDialog` does the same. Repositioning a menu above the keyboard after
an explicit tap is a separate, unsolved problem: Radix collision detection uses
the layout viewport, not `visualViewport`.

## Contributed by

- `vk/6823-model-menu`
