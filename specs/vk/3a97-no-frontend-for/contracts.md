# Contracts: Settings Pipelines Editor

No backend route or response-shape changes are planned. This file records the
existing API contract plus the frontend host-routing and cache contracts the
implementation must honor.

## Existing HTTP API

All routes are registered in `crates/server/src/routes/pipelines.rs` under the
local API prefix and return the standard `ApiResponse` envelope.

| Operation | Method and Path | Request | Success Data |
| --- | --- | --- | --- |
| List selectable pipelines | `GET /api/pipelines` | none | `Pipeline[]` |
| List all file statuses | `GET /api/pipelines/status` | none | `PipelineFileStatus[]` |
| Read raw TOML | `GET /api/pipelines/{id}/raw` | none | `string` |
| Validate draft | `POST /api/pipelines/validate` | `PipelineValidateBody` | `PipelineValidation` |
| Write raw TOML | `PUT /api/pipelines/{id}/raw` | `PipelineRawBody` | `Pipeline` |
| Reset one bundled pipeline | `POST /api/pipelines/{id}/reset` | none | `Pipeline` |
| Reset all bundled defaults | `POST /api/pipelines/reset-defaults` | none | `Pipeline[]` |
| Delete pipeline file | `DELETE /api/pipelines/{id}` | none | `void` |

Path ids must be URL-encoded with `encodeURIComponent`.

## Request Bodies

```ts
type PipelineRawBody = {
  content: string;
};

type PipelineValidateBody = {
  id: string | null;
  content: string;
};
```

Validation from Settings must send the effective file id. Existing files use
the selected id. New drafts use the proposed id.

## Response Types

```ts
type PipelineFileStatus = {
  id: string;
  name: string;
  stage_count: number | null;
  valid: boolean;
  error: PipelineParseError | null;
};

type PipelineValidation = {
  valid: boolean;
  error: PipelineParseError | null;
};

type PipelineParseError = {
  message: string;
  line: number | null;
  column: number | null;
};
```

`line` and `column` are already 1-based when present. The UI must display them
directly and omit location text when either is missing.

## MachineClient Contract

Extend `MachineClient` with host-routed pipeline methods:

```ts
interface MachineClient {
  queryScopeKey: readonly ['machine', string];
  listPipelineStatuses(): Promise<PipelineFileStatus[]>;
  readPipelineRaw(id: string): Promise<string>;
  validatePipeline(body: PipelineValidateBody): Promise<PipelineValidation>;
  writePipelineRaw(id: string, body: PipelineRawBody): Promise<Pipeline>;
  resetPipeline(id: string): Promise<Pipeline>;
  resetDefaultPipelines(): Promise<Pipeline[]>;
  deletePipeline(id: string): Promise<void>;
}
```

These methods must use `makeMachineRequest` so local runtime remote hosts,
remote runtime relay hosts, and local machine requests all follow the existing
Settings host boundary.

The unscoped `pipelinesApi.list()` remains valid only for current-backend
task-create usage. Settings pipeline management must not be implemented by
adding optional host ids to `pipelinesApi` or by calling `makeHostAwareRequest`
from the Settings section.

## Query Key Contract

Settings pipeline keys must include the machine scope:

```ts
const pipelineKeys = {
  statuses: (scope: readonly ['machine', string]) =>
    ['pipeline-statuses', ...scope] as const,
  raw: (scope: readonly ['machine', string], id: string | null) =>
    ['pipeline-raw', ...scope, id] as const,
};
```

Disabled Settings queries use `['machine', 'unselected']` as the scope segment
and `enabled: false`.

Mutation success invalidation:

- invalidate status keys for the same machine scope;
- invalidate raw keys for the same machine scope;
- invalidate legacy `PIPELINES_QUERY_KEY`;
- if task-create becomes host-aware in implementation, invalidate its
  host-aware key using the same scope.

Validation should be a mutation or explicit request, not a persistent query
whose stale result can drive Save state for a later draft.

## UI Boundary Contract

- The Pipelines section is a host-specific Settings section.
- It may read `MachineClient` through `useSettingsMachineClient()`.
- It must not read route host ids directly for API routing.
- It must register dirty state through `SettingsDirtyContext` while
  `draftContent !== lastPersistedContent`.
- Settings host switching must confirm before changing host when any Settings
  section is dirty. A confirmed host switch must clear the discarded dirty
  state before mounted host-scoped sections reseed from the new `MachineClient`;
  a cancelled switch must preserve the current host and draft.
- Pipeline file switching, Add, reset, reset-all, delete, and close/discard
  flows must not discard the current draft without confirmation.

## Localization Contract

Add all user-facing strings under `settings` namespace keys in:

- `packages/web-core/src/i18n/locales/en/settings.json`
- `packages/web-core/src/i18n/locales/es/settings.json`
- `packages/web-core/src/i18n/locales/fr/settings.json`
- `packages/web-core/src/i18n/locales/ja/settings.json`
- `packages/web-core/src/i18n/locales/ko/settings.json`
- `packages/web-core/src/i18n/locales/zh-Hans/settings.json`
- `packages/web-core/src/i18n/locales/zh-Hant/settings.json`

English is authoritative. Non-English files must preserve the same key
structure and may use English fallback strings where native translation is
unavailable.
