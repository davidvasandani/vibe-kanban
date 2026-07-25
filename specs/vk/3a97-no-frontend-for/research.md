# Research: Settings Pipelines Editor

## Inputs Read

- `specs/vk/3a97-no-frontend-for/spec.md`
- `specs/vk/3a97-no-frontend-for/clarifications.md`
- `SPEC.md`
- `PRIOR_KNOWLEDGE.md`
- `IMPLEMENTATION_PLAN.md`
- `assets/speckit/memory/constitution.md`
- Repository files for pipeline routes, generated types, Settings host context,
  Settings dirty state, MachineClient, pipeline hooks, Settings registry, and
  locale structure.

## Decisions

### Host Routing Boundary

Decision: Pipeline Settings reads and writes must go through the Settings
`MachineClient`, not through unscoped `pipelinesApi` calls and not through a
URL-only host-id prop.

Rationale: The project constitution defines host-specific Settings as a data
boundary. Existing host-scoped Settings sections use
`useSettingsMachineClient()` from
`packages/web-core/src/shared/dialogs/settings/settings/SettingsHostContext.tsx`,
and `MachineClient.queryScopeKey` is already the cache identity boundary used
by machine-specific hooks.

Implementation consequence: Extend `MachineClient` in
`packages/web-core/src/shared/lib/machineClient.ts` with typed pipeline methods
implemented via `makeMachineRequest`. Hooks used by Settings should accept
`MachineClient | null`, key with `machineClient.queryScopeKey`, and use
`['machine', 'unselected']` only for disabled queries.

Rejected alternative: Passing a selected host id to `pipelinesApi` from the
Settings component. That would duplicate an existing boundary and risks
accidental calls against the UI machine.

### Unscoped Pipeline Catalog

Decision: Keep `usePipelines()` and `PIPELINES_QUERY_KEY` as the current
unscoped task-create catalog for this feature, and invalidate it after every
successful pipeline file mutation.

Rationale: The current task-create `PipelineSection` consumes
`usePipelines()` from `packages/web-core/src/shared/hooks/usePipelines.ts`,
which uses `PIPELINES_QUERY_KEY = ['pipelines']`. The spec explicitly does not
require making task-create host-aware in this feature, but it does require
task-create to stop showing stale definitions after Settings mutations.

Implementation consequence: New Settings mutations must invalidate both
host-scoped Settings keys and the legacy `PIPELINES_QUERY_KEY`. If later
implementation opportunistically makes task-create host-aware, the host-aware
query key must also include the same `MachineClient.queryScopeKey`.

### Validation Identity

Decision: Draft validation must always include an id: selected file id for
existing files and proposed file id for new drafts.

Rationale: `POST /api/pipelines/validate` accepts `PipelineValidateBody` with
`id: string | null`; the server falls back to `"pipeline"` only when omitted.
Clarifications require id-specific failures to surface before save, so omitting
id would hide conflicts or slug errors tied to the real file id.

### Validation Race Handling

Decision: Treat validation state as belonging to a `(machine scope, pipeline id,
content)` tuple. A validation result may update visible state or Save
availability only if it matches the latest tuple.

Rationale: Settings can switch hosts while mounted, file selection can change
while a debounce is active, and content can change while a request is in flight.
React Query mutation state alone does not prove a validation result still
belongs to the visible draft.

Implementation consequence: Store a monotonically increasing validation request
token or tuple ref in the section. On response, compare against the current
tuple before applying the result. Save must validate or await validation for the
exact content being written.

### Dirty State And Host Switching

Decision: Use the existing `SettingsDirtyContext` for dialog-close protection,
and add a host-switch confirmation path in Settings navigation for dirty
sections. Pipeline file switches, Add, reset/delete actions, and local discard
flows remain guarded inside the Pipelines section.

Rationale: `SettingsDialog` currently confirms on close, but
`SettingsDialogNavigation` calls `setSelectedHostId` directly. The constitution
requires host switching not to leak stale drafts or mutate the wrong host.

Implementation consequence: Settings navigation needs an async host-selection
handler that asks the existing `ConfirmDialog` before changing host when
`isDirty` is true. After confirmed host change, clear dirty state or rely on
the Pipelines section to clear its dirty flag during host reseed. The section
must also clear selected id, raw content, validation result, and mutation error
when `machineClient.queryScopeKey` changes.

### Localization

Decision: Add English copy under the existing `settings` namespace and mirror
the full key structure in `es`, `fr`, `ja`, `ko`, `zh-Hans`, and `zh-Hant`,
using English fallback strings where native translations are not available.

Rationale: Existing Settings strings live in
`packages/web-core/src/i18n/locales/*/settings.json`, and the clarification
requires structurally complete locale files.

### Dependencies

Decision: No new top-level dependencies.

Rationale: The feature can use existing React, TanStack Query, Settings UI,
`ConfirmDialog`, and browser textarea behavior. Raw TOML editing does not
require adding a parser or editor dependency because validation is delegated to
the existing backend endpoint.

## Open Risks

- Task-create remains on a legacy unscoped cache key unless later work makes it
  host-aware. This plan mitigates stale data by invalidating
  `PIPELINES_QUERY_KEY` after mutations.
- The server currently reseeds bundled defaults after the final pipeline is
  deleted on a later list/read cycle. This feature must document that behavior
  in UI copy, not change it.
