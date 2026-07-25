# Implementation Plan: Settings Pipelines Editor

1. **Confirm contracts and conventions**
   - Treat the routes actually registered in
     `crates/server/src/routes/pipelines.rs` (`/status` and `/{id}/raw`) as the
     source of truth.
   - Reuse generated `PipelineFileStatus`, `PipelineValidation`,
     `PipelineRawBody`, and `PipelineValidateBody` types.
   - Follow the existing Settings host context and machine-aware API transport.

2. **Extend the machine-aware frontend client**
   - Import the pipeline management types into
     `packages/web-core/src/shared/lib/machineClient.ts`.
   - Add status, raw read, validate, write, reset-one, reset-all, and delete
     methods to `MachineClient`.
   - Implement each Settings management request with `makeMachineRequest`, so
     local runtime remote hosts, remote runtime relay hosts, and local machine
     requests preserve the existing Settings host boundary.
   - Keep `pipelinesApi.list()` in `shared/lib/api.ts` unscoped for the current
     task-create catalog.
   - Encode file ids as path segments.

3. **Add query and mutation hooks**
   - Extend `shared/hooks/usePipelines.ts` with host-keyed status/raw queries
     and the requested mutation hooks.
   - Centralize query keys so raw data is keyed by both host and pipeline id.
   - On successful writes/resets/deletes, invalidate status queries, raw
     queries, and the existing task-create pipeline catalog.
   - Keep validation a mutation so debounced drafts do not pollute persistent
     query cache state.

4. **Build the Settings editor**
   - Add pure helpers for supported ids, bundled ids, error-location display,
     and the initial TOML template, with focused tests.
   - Create `PipelinesSettingsSection.tsx` using the Settings host context.
   - Implement file selection, raw loading, new-draft creation, dirty tracking,
     debounced validation, explicit save validation, and visible API feedback.
   - Render malformed status entries and draft errors with optional 1-based
     line/column.
   - Add confirmed delete, per-bundled reset, and reset-all actions.
   - Guard file/host switches when a dirty draft would be discarded, and clear
     stale draft state on accepted transitions.

5. **Register and localize the section**
   - Add a host-scoped `pipelines` section and icon to
     `settingsRegistry.tsx`.
   - Render the new section and update initial-state typing.
   - Add navigation and editor strings to all Settings locale JSON files,
     providing fully translated English and repository-consistent fallback
     text for the other locales.

6. **Verify incrementally**
   - Run focused frontend tests for helper behavior.
   - Run web-core type checking and linting, fixing only issues caused by this
     change.
   - Run required repository formatting and inspect the resulting diff for
     unrelated changes.
   - If the local app can be started reliably, smoke-test status display,
     invalid line/column, create/save/delete, reset-one, and reset-all.

7. **SpecKit execution**
   - Reconcile SpecKit artifacts with `SPEC.md`, `PRIOR_KNOWLEDGE.md`, and this
     implementation plan rather than allowing duplicate artifacts to diverge.
   - Do not rerun completed SpecKit stages unless explicitly requested.
   - During implementation, tick tasks as each dependency layer lands.

8. **Independent review and closure**
   - Run the repository's Codex review workflow against the complete diff.
   - Address confirmed significant findings, rerun focused verification, and
     repeat review until no significant findings remain.
   - Distill reusable file-backed Settings editor knowledge into the project
     knowledge base, update its index with task `3a97-no-frontend-for`, and
     commit the knowledge-base update as the pipeline requires.
