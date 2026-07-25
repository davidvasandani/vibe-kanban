# Feature Specification: Settings Pipelines Editor

**Feature directory**: `specs/vk/3a97-no-frontend-for/`  
**Task**: `3a97-no-frontend-for`  
**Status**: Clarified

## Summary

Add a host-scoped **Pipelines** section to the existing Settings dialog so an
operator can manage pipeline TOML files for the selected machine. The section
allows users to discover valid and invalid pipeline files, inspect and edit raw
TOML, validate drafts before saving, create new pipeline files, delete existing
files, and reset bundled default pipelines.

The feature exposes existing pipeline-file management behavior through the
frontend. Pipeline TOML remains the authoritative source; the editor must
preserve raw content and avoid hidden rewrites.

## Why

Pipeline files currently influence task creation, prompt composition, and stage
execution, but users do not have a Settings UI for inspecting or correcting the
underlying TOML files. Invalid files are especially difficult to diagnose
because malformed pipelines can be excluded from normal pipeline selection while
still existing on disk.

Providing a Settings editor gives operators a single place to manage the
selected host's pipeline catalog, fix parse or validation errors, and restore
bundled defaults without leaving the app or editing files manually.

## User Stories

- As an operator managing a local or remote host, I want to view all pipeline
  files for the selected host so I can understand what that host can use during
  task creation.
- As an operator troubleshooting a missing pipeline, I want invalid TOML files
  to appear with their validation errors so I can repair them.
- As an operator editing a pipeline, I want raw TOML editing with validation
  feedback before save so I can make precise changes without corrupting the
  catalog.
- As an operator creating a new workflow, I want to add a pipeline id and start
  from a minimal valid TOML draft so I can define a new pipeline intentionally.
- As an operator recovering from a bad edit, I want to reset one bundled
  pipeline or all bundled defaults so I can return to known-good definitions.
- As an operator cleaning up obsolete workflows, I want to delete a pipeline
  file only after confirmation so accidental deletion is avoided.
- As a user creating tasks after a pipeline edit, I want the task-create
  pipeline list to reflect saved changes immediately.

## Functional Requirements

- **FR-1**: Settings MUST include a host-specific **Pipelines** section in the
  existing Settings navigation.
- **FR-2**: All pipeline requests from the Settings section MUST apply to the
  currently selected Settings host, not implicitly to the UI machine.
- **FR-3**: The pipeline list MUST show every pipeline file reported by the host,
  including malformed `.toml` files that are not selectable for task creation.
- **FR-4**: Each listed file MUST identify the pipeline by stable pipeline id,
  because display names are not unique.
- **FR-5**: Each listed file MUST show whether it is valid or invalid.
- **FR-6**: For valid files, the list MUST show the known stage count when the
  host provides it.
- **FR-7**: For invalid files, the list MUST show the structured validation or
  parse message and MUST show 1-based line and column positions when available.
- **FR-8**: Opening the Pipelines section MUST select a sensible initial file,
  preferring the prior selection when that file still exists for the selected
  host and otherwise selecting the first status in server-provided order.
- **FR-9**: Selecting an existing file MUST load and display that file's raw TOML
  exactly as returned by the host.
- **FR-10**: The editor MUST provide a multiline monospace raw TOML editing
  surface.
- **FR-11**: The editor MUST validate drafts after a short debounce and again
  when the user explicitly saves.
- **FR-11a**: Validation MUST use the selected existing file id or the proposed
  new file id so id-specific validation failures are surfaced before save.
- **FR-12**: Validation feedback MUST distinguish pending, valid, and invalid
  states.
- **FR-13**: Invalid validation feedback MUST include the structured error
  message and available 1-based line and column positions.
- **FR-14**: Save MUST be disabled while validation is pending, when the draft is
  invalid, or when the raw TOML is unchanged from the last loaded or saved value.
- **FR-15**: Saving MUST write only validated TOML and MUST keep the saved file
  selected after success.
- **FR-16**: After create, save, delete, or reset, the section MUST refresh the
  pipeline file statuses and the task-create selectable pipeline catalog so
  users do not see stale stage definitions.
- **FR-17**: The Add action MUST require a valid pipeline id before opening a new
  draft.
- **FR-18**: A new pipeline draft MUST start from a valid one-stage TOML
  template using the proposed pipeline id as the initial display name and
  `stage-1` as the placeholder stage id.
- **FR-19**: Creating a new pipeline MUST NOT create a file on the host until the
  user saves the draft.
- **FR-19a**: The Add flow MUST reject an id that exactly matches an existing
  pipeline status id for the selected host before opening a new draft.
- **FR-20**: The editor MUST protect unsaved draft content when the user switches
  files, changes selected host, starts a new file, closes the relevant flow, or
  begins any action that would discard the draft.
- **FR-21**: The section MUST offer per-file reset for bundled pipeline ids
  `basic`, `wikillm`, `speckit`, and `parallel-subagents`.
- **FR-22**: Per-file reset MUST require confirmation and MUST communicate that
  the file content will be overwritten.
- **FR-23**: The section MUST offer a reset-all-defaults action that recreates or
  overwrites all bundled pipeline files after confirmation.
- **FR-24**: The section MUST offer delete for existing files only after
  confirmation.
- **FR-25**: After deleting the selected file, the section MUST refresh file
  statuses and select another available file when one exists.
- **FR-26**: The UI MUST explain the existing server behavior where deleting the
  final pipeline can cause bundled defaults to be seeded again on a later
  list/read cycle.
- **FR-27**: Loading, empty, validation, mutation-pending, and request-failure
  states MUST be explicit to the user.
- **FR-28**: Mutation failures MUST remain visible in the Pipelines section and
  MUST NOT discard the current draft.
- **FR-29**: Opening the Settings dialog, switching Settings host, or reusing a
  mounted Settings component MUST NOT leak stale drafts, selected pipeline ids,
  validation state, or server data from another host.
- **FR-30**: The feature MUST use existing frontend request/error handling,
  confirmation conventions, Settings visual patterns, and translation
  namespaces.
- **FR-31**: The Settings implementation MUST use the existing
  `MachineClient`/Settings host boundary for pipeline reads and writes, with
  cache keys derived from the selected machine scope.
- **FR-32**: Validation results MUST only update visible state or Save
  availability when they correspond to the latest selected host, pipeline id,
  and editor content.

## Edge Cases

- The selected host has no user-created pipeline files.
- The selected host has only invalid pipeline files.
- The selected pipeline is deleted, reset, or disappears after a refresh.
- A user edits a file while validation is still pending.
- A user attempts to save unchanged content.
- A user creates a new draft and then switches to another file or host before
  saving.
- A new pipeline id conflicts with an existing file.
- A validation response returns after the user has changed host, selected a
  different pipeline, or edited the content again.
- A pipeline display name duplicates another pipeline's display name.
- A validation error includes a message but no line or column.
- A network or host-routing failure occurs while loading raw TOML, validating,
  saving, deleting, or resetting.
- Reset-all changes the file currently open in the editor.
- Deleting the final pipeline triggers the host's default seeding behavior on a
  later list/read cycle.
- The Settings dialog is reopened with components still mounted from a prior
  session.

## Acceptance Criteria

- [ ] Given the user opens Settings for host A, when they open **Pipelines**,
      then the section lists host A's pipeline file statuses, not files from host
      B or the UI machine.
- [ ] Given a malformed pipeline file exists, when statuses load, then the file
      appears in the list with an invalid state, error message, and available
      1-based line/column information.
- [ ] Given a valid pipeline file exists, when statuses load, then the file
      appears with its stable id, valid state, and known stage count.
- [ ] Given a user selects an existing pipeline, when the raw file loads, then
      the editor displays the TOML without implicit formatting or rewriting.
- [ ] Given the user edits TOML into an invalid draft, when validation completes,
      then Save is disabled and the validation message remains visible.
- [ ] Given the user edits TOML into a valid changed draft, when validation
      completes, then Save becomes available and saving refreshes statuses, raw
      content, and the task-create pipeline catalog.
- [ ] Given the user edits nothing after loading a file, then Save remains
      disabled.
- [ ] Given the user starts adding a pipeline with a valid new id, then no host
      file is created until the user saves the valid draft.
- [ ] Given the user starts adding a pipeline with an id that already exists on
      the selected host, then the UI rejects the id before opening a draft.
- [ ] Given validation for older editor content returns after the user edits
      again or changes host/file, then that stale validation result does not
      change Save availability or visible validation state.
- [ ] Given the user has unsaved changes, when they attempt to switch files,
      start a new draft, or change host, then the UI asks for confirmation before
      discarding the draft.
- [ ] Given the selected file is one of the bundled pipeline ids, when the user
      resets it and confirms, then the host overwrites that file with the
      bundled default and the editor/list refresh.
- [ ] Given the user chooses reset all defaults and confirms, then all bundled
      defaults are recreated or overwritten and the editor/list refresh.
- [ ] Given reset all defaults would overwrite the file currently open in the
      editor, when the user has unsaved changes, then the confirmation explains
      both the default overwrite and draft discard before proceeding.
- [ ] Given the user deletes a file and confirms, then the file is removed,
      statuses refresh, and another file is selected if available.
- [ ] Given the user is deleting the final pipeline file, then the confirmation
      or adjacent UI copy explains that bundled defaults may be seeded again on
      a later list/read cycle.
- [ ] Given a save, delete, validation, or reset request fails, then the error is
      shown and any unsaved draft content remains intact.
- [ ] Given the user has an unsaved new-pipeline draft, when they close the
      local add/draft flow, then the UI asks for confirmation before discarding
      it.
- [ ] Given the Settings dialog is closed and reopened or reused while mounted,
      then the Pipelines section seeds state from the current host and selected
      file rather than stale prior draft state.
- [ ] Given pipeline files were created, saved, deleted, or reset, then the
      task-create pipeline selector no longer shows stale pipeline definitions.

## Non-Functional Requirements

- The feature MUST NOT require backend route or response-shape changes.
- The feature MUST NOT change pipeline TOML semantics or persistence rules.
- The editor MUST remain usable and readable at normal Settings dialog sizes,
  including narrow layouts.
- Pipeline query/cache identity MUST include host identity to prevent
  cross-host data display.
- Pipeline query/cache identity MUST use the existing Settings machine scope
  (`MachineClient.queryScopeKey`) for Settings data and invalidate the legacy
  `PIPELINES_QUERY_KEY` until all task-create consumers use host-aware keys.
- Draft ownership MUST remain in the Settings section while persisted pipeline
  data remains in the existing query layer.
- Automated verification SHOULD cover pure helper behavior for pipeline-id
  validation, location formatting, selection after mutations, host-aware query
  keys, and stale validation-result rejection where practical.

## Out of Scope

- A structured form editor for pipeline stages.
- Backend persistence changes or new pipeline API routes.
- Changing bundled default pipeline contents.
- Editing files outside the selected Settings host.
- Fixing the existing all-or-nothing default seeding behavior tracked
  separately as VAS-225.
- Persisting draft editor state across Settings sessions.

## Assumptions

- Existing pipeline management endpoints are sufficient for list, raw read,
  validate, write, reset, reset-all, and delete operations.
- Pipeline ids are server-validated slugs containing ASCII alphanumerics,
  hyphens, or underscores.
- The existing Settings dialog already has a selected-host concept that other
  host-specific sections use.
- English copy is authoritative; other locales remain structurally complete
  according to the repository's translation conventions.
- The new-pipeline starter template is a valid one-stage TOML draft using the
  proposed pipeline id as the display name and a placeholder `stage-1`.

## Clarifications

See `clarifications.md` in this feature directory for the resolved decisions
from the clarify stage, including host-scoped API routing, validation id
handling, new-draft template shape, selection behavior, mutation invalidation,
and locale-key expectations.
