# Settings Pipelines Editor — Technical Specification

## Summary

Add a host-scoped **Pipelines** section to the existing Settings dialog so an
operator can manage the TOML files in `~/.vibe-kanban/pipelines` through the
already-implemented pipeline HTTP API. The section must list valid and invalid
files, edit and validate raw TOML, create and delete files, and reset bundled
pipelines individually or as a group.

This is a frontend integration. No backend behavior or route shape should
change unless implementation proves an existing route cannot satisfy the
requirements.

## Existing Contract

The implementation must use the routes currently registered by
`crates/server/src/routes/pipelines.rs`:

| Operation | Route | Result |
| --- | --- | --- |
| List selectable pipelines | `GET /api/pipelines` | `Pipeline[]` |
| List every file and validity | `GET /api/pipelines/status` | `PipelineFileStatus[]` |
| Read raw TOML | `GET /api/pipelines/{id}/raw` | `string` |
| Write validated TOML | `PUT /api/pipelines/{id}/raw` | `Pipeline` |
| Validate a draft | `POST /api/pipelines/validate` | `PipelineValidation` |
| Reset one bundled file | `POST /api/pipelines/{id}/reset` | `Pipeline` |
| Reset all bundled files | `POST /api/pipelines/reset-defaults` | `Pipeline[]` |
| Delete a file | `DELETE /api/pipelines/{id}` | empty success |

All endpoints use the standard `ApiResponse` envelope. Pipeline ids are
server-validated slugs containing ASCII alphanumerics, `-`, or `_`.

## User Experience

### Navigation and scope

- Add `Pipelines` to the machine/host-specific Settings navigation.
- Requests follow the Settings dialog's selected-host routing behavior.
- Opening the section loads file statuses and selects a sensible initial file,
  preferring the existing selection when it remains available.

### File list

- Show every `.toml` file returned by the status endpoint, including malformed
  files omitted by `GET /api/pipelines`.
- Each item shows its display name/id, validity, and stage count when known.
- Invalid items show the structured parse/validation message. When supplied,
  line and column are displayed as 1-based positions.
- Provide an Add action that asks for a valid pipeline id and opens a new draft
  based on a minimal valid TOML template. A new file is created only on Save.

### Editor

- Read the selected existing file's raw TOML from the raw endpoint.
- Use a multiline monospace text editor suitable for plain TOML.
- Validate drafts after a short debounce and on explicit Save.
- Show valid/invalid state inline. Invalid state includes message and available
  line/column from `PipelineParseError`.
- Disable Save while the current validation is pending or invalid, and avoid a
  write when the editor is unchanged.
- A successful Save refreshes statuses, the task-create pipeline list, and the
  current raw query; the saved file remains selected.
- Protect unsaved editor content when switching files or starting a new file
  with a confirmation prompt.

### Reset and delete

- Offer per-file reset for bundled ids (`basic`, `wikillm`, `speckit`,
  `parallel-subagents`) and require confirmation because it overwrites content.
- Offer a global Reset all defaults action and require confirmation. This
  recreates/overwrites all bundled pipeline files, then refreshes list/editor
  data.
- Offer Delete for existing files and require confirmation. After deletion,
  refresh list data and select another file.
- Explain the existing server edge case: deleting the final pipeline causes
  bundled defaults to be seeded again on the next list/read cycle.

### Errors and loading

- Loading, empty, and request-failure states must be explicit.
- Mutating action errors remain visible in the section and do not discard the
  draft.
- API errors use the existing frontend request/error handling.

## Frontend Design

### API and hooks

Keep `pipelinesApi.list()` as the unscoped current-backend task-create catalog.
Add Settings management methods to `MachineClient` and implement them with the
existing machine-aware `makeMachineRequest` transport:

- `listPipelineStatuses`
- `readPipelineRaw`
- `validatePipeline`
- `writePipelineRaw`
- `resetPipeline`
- `resetDefaultPipelines`
- `deletePipeline`

Extend `usePipelines.ts` with:

- `usePipelineStatuses`
- `usePipelineRaw`
- `useValidatePipeline`
- `useWritePipeline`
- `useResetPipeline`
- `useResetDefaults`
- `useDeletePipeline`

Settings queries and mutations must accept `MachineClient | null`, key data
with `MachineClient.queryScopeKey`, and use `['machine', 'unselected']` only for
disabled queries. Successful file mutations must invalidate host-scoped status
and raw queries plus the legacy `PIPELINES_QUERY_KEY`. The Settings section must
not route these calls through URL-only host ids or `makeHostAwareRequest`.

### Settings section

Create a focused `PipelinesSettingsSection` component and register it in
`settingsRegistry.tsx` as a host-specific section. Reuse existing Settings
cards, buttons, confirmation conventions, colors, and typography. Add English
translation keys and keep every other locale structurally complete with
English fallback strings where this repository's translation convention
requires keys in every locale.

### State

The component owns:

- selected pipeline id and new-draft id;
- current raw draft and last-loaded/saved raw value;
- debounced validation result;
- mutation feedback and pending state.

Server state remains in TanStack Query. Changing the selected Settings host
must produce host-specific query keys and fresh data rather than showing data
from another host.

## Verification

- Unit-test any extracted pure helpers (id validation, location formatting, or
  selection behavior) and query-key behavior where practical.
- Run frontend type checking and relevant tests.
- Run repository formatting as required by `AGENTS.md`.
- Exercise the complete UI/API path where the local development environment
  permits it, including an invalid draft with line/column, create, save,
  delete, reset one, and reset all.

## Out of Scope

- Changing pipeline TOML semantics or backend persistence.
- A structured stage/form editor; raw TOML is authoritative.
- Editing pipeline files outside the selected host.
- Solving the separate all-or-nothing seeding behavior tracked by VAS-225.
