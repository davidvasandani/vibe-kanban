# Technical Plan: Settings Pipelines Editor

## Scope

Build a host-scoped Pipelines section in the existing Settings dialog. The
section exposes the existing file-backed pipeline API for status listing, raw
TOML read/write, draft validation, delete, bundled reset, and reset-all. This
plan does not add backend routes, alter pipeline TOML semantics, create tasks,
or run later SpecKit stages.

## Constitution Check

- Clarity over cleverness: use existing Settings, MachineClient, TanStack
  Query, and confirmation patterns.
- Test the contract: plan includes focused tests for helper logic, query keys,
  validation races, dirty/host switching, and localization shape.
- Small, reversible steps: frontend-only integration around existing endpoints.
- One MCP contract: not applicable.
- Settings host scope is a data boundary: this is the primary design boundary;
  all Settings pipeline reads, writes, cache keys, and draft ownership are tied
  to the Settings-selected `MachineClient`.

No constitution violations.

## Existing Architecture

- Backend routes already exist in `crates/server/src/routes/pipelines.rs`.
- Generated TypeScript types already exist in `shared/types.ts`.
- Settings host selection lives in
  `packages/web-core/src/shared/dialogs/settings/settings/SettingsHostContext.tsx`.
- Host-specific Settings code should use
  `useSettingsMachineClient()` and `MachineClient.queryScopeKey`.
- `MachineClient` currently has host-routed methods for config, repos,
  profiles, MCP, CLI tools, and AWS in
  `packages/web-core/src/shared/lib/machineClient.ts`.
- The current task-create catalog uses unscoped `usePipelines()` and
  `PIPELINES_QUERY_KEY` in
  `packages/web-core/src/shared/hooks/usePipelines.ts`.
- Settings close protection uses `SettingsDirtyContext`, but host switching
  currently calls `setSelectedHostId` directly from navigation.
- Settings locale files live under
  `packages/web-core/src/i18n/locales/*/settings.json`.

## Technical Design

### API And Host Routing

Extend `MachineClient` with pipeline management methods and implement them
with `makeMachineRequest`:

- `listPipelineStatuses`
- `readPipelineRaw`
- `validatePipeline`
- `writePipelineRaw`
- `resetPipeline`
- `resetDefaultPipelines`
- `deletePipeline`

Keep `pipelinesApi.list()` unscoped for the current task-create flow. Do not
use `makeHostAwareRequest` or route params from the Pipelines Settings section;
the Settings-selected `MachineClient` is the boundary.

### Query And Mutation Layer

Extend `packages/web-core/src/shared/hooks/usePipelines.ts` with Settings-safe
hooks:

- status query keyed by `['pipeline-statuses', ...queryScopeKey]`;
- raw query keyed by `['pipeline-raw', ...queryScopeKey, pipelineId]`;
- validation mutation for debounced and explicit validation;
- write, reset-one, reset-all, and delete mutations.

Mutation success must invalidate:

- status queries for the same machine scope;
- raw queries for the same machine scope;
- legacy `PIPELINES_QUERY_KEY`.

If task-create becomes host-aware during implementation, add invalidation for
the host-aware task-create pipeline key too.

### Settings Section

Add a `PipelinesSettingsSection` under the existing Settings sections and
register it as `group: 'host'` in `settingsRegistry.tsx`.

The section owns local draft state:

- selected pipeline id;
- draft kind (`existing` or `new`);
- new draft id;
- raw draft content;
- last loaded or saved content;
- validation state for the latest `(scope, id, content)` tuple;
- mutation error and pending state.

Persisted server state remains in TanStack Query.

The UI should reuse Settings visual patterns: section cards, existing buttons,
`ConfirmDialog`, compact list/editor layout, and explicit loading/empty/error
states. The raw editor should be a multiline monospace editing surface that
writes the user string unchanged.

### Selection Behavior

On status refresh for the same host:

- keep the selected id if it still exists;
- otherwise select the first status in server order;
- if loading failed or the status list is empty, use no selection.

Opening/reopening Settings or switching host must seed from the current
machine scope, not from stale selected ids or drafts left in a mounted
component.

### Dirty State And Host Switching

The Pipelines section must set its Settings dirty flag when
`draftContent !== lastPersistedContent` and clear it after save, discard, or
host reseed.

Host switching needs a confirmation guard in Settings navigation when any
section is dirty. Confirmed host switches proceed only after the Settings dirty
context is cleared for the discarded draft state, then mounted sections reseed
from the new `MachineClient`; cancelled switches keep the current host and
draft intact.

Within the Pipelines section, confirmation is also required before actions that
discard or overwrite the current draft:

- selecting another file;
- starting Add;
- deleting the selected file;
- resetting the selected bundled file;
- reset-all when the open draft would be overwritten;
- closing a local flow that would discard a new draft.

Actions against another file do not discard the draft unless the post-mutation
selection algorithm changes the current selection.

### Validation Race Handling

Debounced validation and save validation both send the effective id and raw
content. The section must apply validation results only when they match the
latest machine scope, pipeline id, and content.

Save is enabled only when the latest tuple is valid, no validation is pending,
no write is pending, and content differs from the last persisted value. Save
must perform or await validation for the exact content being written.

### Add, Reset, And Delete

Add flow:

- require an id matching ASCII alphanumeric, hyphen, or underscore slug rules;
- reject exact conflicts with existing status ids for the selected host;
- open an unsaved new draft with the clarified one-stage TOML template;
- create no host file until Save.

Reset:

- per-file reset is available for `basic`, `wikillm`, `speckit`, and
  `parallel-subagents`;
- reset-one and reset-all require confirmation and refresh list/raw/catalog
  data after success.

Delete:

- only existing files can be deleted;
- delete requires confirmation;
- after deleting the selected file, refresh statuses and select another file
  when available;
- UI copy must mention the existing default-reseeding behavior when deleting
  the final pipeline.

### Localization

Add navigation and Pipelines section strings under the existing `settings`
namespace. Mirror the complete key structure in `es`, `fr`, `ja`, `ko`,
`zh-Hans`, and `zh-Hant`, using English fallback text where native translation
is unavailable.

## File Impact

Expected implementation files:

- `packages/web-core/src/shared/lib/machineClient.ts`
- `packages/web-core/src/shared/hooks/usePipelines.ts`
- `packages/web-core/src/shared/dialogs/settings/SettingsDialog.tsx`
- `packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx`
- `packages/web-core/src/shared/dialogs/settings/settings/PipelinesSettingsSection.tsx`
- helper module under
  `packages/web-core/src/shared/lib/pipeline/pipelineSettings.ts`
- focused Vitest files for pure helpers and key/race behavior
- `packages/web-core/src/i18n/locales/*/settings.json`

Generated `shared/types.ts` must not be edited.

## Verification Plan

Automated checks to add or run during implementation:

- Pure helper tests for:
  - pipeline id validation;
  - bundled id detection;
  - location formatting with and without line/column;
  - initial template generation;
  - post-refresh selection algorithm.
- Query/key tests where practical:
  - status and raw keys include `MachineClient.queryScopeKey`;
  - disabled keys use `['machine', 'unselected']`;
  - mutation success invalidates host-scoped keys and legacy
    `PIPELINES_QUERY_KEY`.
- Validation race tests for stale host/file/content responses not updating
  visible validation state or Save availability.
- Dirty/host-switch behavior tests or focused component tests where practical:
  - dirty draft blocks host switch until confirmed;
  - cancelled switch preserves host and draft;
  - confirmed switch reseeds state from the new host.
- Localization shape check:
  - all locale `settings.json` files contain the new key structure.

Manual smoke coverage when implementation reaches UI verification:

- Host A versus host B status loading.
- Invalid file row with message and 1-based line/column.
- Existing raw TOML loads without reformatting.
- Invalid draft disables Save.
- Valid changed draft saves and refreshes status/raw/task-create catalog.
- Add creates no file until Save.
- Delete, reset-one, and reset-all confirm and refresh.
- Mutation failures remain visible and preserve draft content.

Repository commands for implementation completion:

- `pnpm --filter @vibe/web-core run test -- --runInBand` if test concurrency
  proves flaky; otherwise `pnpm --filter @vibe/web-core run test`
- `pnpm run check`
- `pnpm run lint`
- `pnpm run format`

## Risks And Mitigations

- Cross-host data leakage: bind all Settings queries, mutations, draft reset,
  and validation tuples to `MachineClient.queryScopeKey`.
- Stale task-create catalog: invalidate legacy `PIPELINES_QUERY_KEY` after all
  successful Settings mutations.
- Validation races: compare response tuple against current scope/id/content
  before applying validation state.
- Dirty draft loss on host switch: add confirmation at Settings navigation and
  keep section-local guards for file/action transitions.
- Locale drift: update every existing Settings locale file with matching keys.

## Out Of Scope

- Backend route or response-shape changes.
- Pipeline TOML semantics or bundled default content changes.
- Structured stage editor.
- Editing files outside the selected Settings host.
- Fixing the default reseeding behavior tracked separately as VAS-225.
- Implementing product code or running later SpecKit stages from this analysis
  pass.
