# Data Model: Settings Pipelines Editor

This feature does not add database tables or backend persistence structures.
Pipeline TOML files remain the source of truth on the selected machine. The
frontend model below describes the state needed to safely expose existing
pipeline file operations in Settings.

## Server Types

The implementation reuses generated types from `shared/types.ts`.

### `Pipeline`

Selectable, valid pipeline returned by `GET /api/pipelines`.

- `id: string`: stable file stem and pipeline identity.
- `name: string`: display name from TOML; not unique.
- `description: string | null`
- `stages: PipelineStep[]`

Usage: task-create catalog and successful write/reset responses.

### `PipelineFileStatus`

Status row returned by `GET /api/pipelines/status`.

- `id: string`: stable identity and list key.
- `name: string`: display name or fallback name from host.
- `stage_count: number | null`: known for valid parsed files.
- `valid: boolean`
- `error: PipelineParseError | null`: present for invalid rows.

Usage: Settings file list, Add conflict checks, initial selection, and
post-mutation selection.

### `PipelineParseError`

Structured parse or validation problem.

- `message: string`
- `line: number | null`: already 1-based when present.
- `column: number | null`: already 1-based when present.

Usage: invalid status rows and draft validation feedback. If either `line` or
`column` is absent, omit the location fragment.

### `PipelineValidation`

Draft validation result returned by `POST /api/pipelines/validate`.

- `valid: boolean`
- `error: PipelineParseError | null`

Usage: Save enablement and inline validation feedback.

## Frontend State

### Machine Scope

- `machineClient: MachineClient | null`
- `scopeKey: readonly ['machine', string]`

All Settings pipeline queries and mutations are scoped by
`MachineClient.queryScopeKey`. Disabled queries use
`['machine', 'unselected']`.

### Selected File

- `selectedPipelineId: string | null`
- `draftKind: 'existing' | 'new' | null`
- `newDraftId: string | null`

Selection keys use pipeline ids only. Display names are not unique and must not
be used as identifiers.

Selection after status refresh:

- Keep the current selected id when it still exists for the same host.
- Otherwise select the first `PipelineFileStatus` in server order.
- If status loading fails or no statuses are available, use no selection.

After deleting the selected file, refresh statuses and apply the same
selection algorithm.

### Raw Draft

- `draftContent: string`
- `lastPersistedContent: string`
- `isDirty: boolean`

For existing files, both strings seed from `GET /api/pipelines/{id}/raw`.
For new files, `draftContent` seeds from the starter template and
`lastPersistedContent` is empty or a sentinel that makes the draft dirty.
Saving writes `draftContent` unchanged.

Starter template:

```toml
name = "<pipeline-id>"
description = ""

[[stage]]
id = "stage-1"
label = "Stage 1"
prompt = "Describe what this stage should do."
```

### Validation State

- `validationTuple: {
    scopeKey: readonly ['machine', string] | string;
    id: string;
    content: string;
  }`
- `validationStatus: 'idle' | 'pending' | 'valid' | 'invalid'`
- `validationError: PipelineParseError | null`

Only results matching the latest tuple may update visible validation state or
Save availability. Implementations may store a serialized scope key internally,
but comparisons must distinguish hosts. Save must validate or reuse a valid
result for the exact current tuple.

### Mutation Feedback

- `mutationPending: boolean`
- `mutationError: string | null`

Mutation errors remain visible and do not discard `draftContent`.

## Derived Rules

- Save is enabled only when:
  - `machineClient` exists;
  - a draft id exists;
  - `draftContent !== lastPersistedContent`;
  - validation is valid for the latest tuple;
  - no validation or write mutation is pending.
- Add can open a new draft only when:
  - proposed id matches server-compatible slug expectations:
    ASCII alphanumeric, `_`, or `-`;
  - proposed id does not exactly match an existing status id for the selected
    host.
- Per-file reset is available only for bundled ids:
  - `basic`
  - `wikillm`
  - `speckit`
  - `parallel-subagents`
- Delete is available only for existing files, not unsaved new drafts.
