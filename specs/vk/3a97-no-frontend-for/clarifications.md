# Clarifications: Settings Pipelines Editor

`/speckit.clarify` found no blocking open questions after comparing the feature
spec with `SPEC.md`, `PRIOR_KNOWLEDGE.md`, the repository, and the project
constitution. The decisions below resolve material ambiguity for later SpecKit
stages without expanding scope beyond the frontend integration.

## Resolved decisions

1. **How is Settings host scope applied?** Use the existing Settings
   `MachineClient` boundary from `SettingsHostContext`, and extend it with
   pipeline methods. Pipeline Settings hooks should key queries with
   `machineClient.queryScopeKey`, using `['machine', 'unselected']` only for
   disabled queries.
2. **Should `pipelinesApi` stay unscoped?** Keep the existing unscoped
   `pipelinesApi.list()` for non-Settings current-backend usage, but add typed
   pipeline methods that can be called through `MachineClient` for Settings.
   Do not rely on URL-only host scoping from the Settings section.
3. **What must happen to the task-create pipeline selector?** Its pipeline
   catalog cache must be invalidated after any successful pipeline mutation for
   the same machine scope. If implementation makes the task-create hook
   host-aware, its query key must include the same host identity; otherwise
   invalidate the legacy `PIPELINES_QUERY_KEY` as well to avoid stale task
   composition.
4. **Which id is used for validation?** Existing-file validation uses the
   selected file id. New-draft validation uses the proposed file id. This is
   required because `POST /api/pipelines/validate` validates both the TOML and
   the supplied id; omitting `id` would validate against the endpoint's
   placeholder slug and miss id-specific failures.
5. **What is the new pipeline template?** A new draft starts with a valid,
   editable one-stage TOML template using the chosen id as the display name:

   ```toml
   name = "<pipeline-id>"
   description = ""

   [[stage]]
   id = "stage-1"
   label = "Stage 1"
   prompt = "Describe what this stage should do."
   ```

6. **How are add conflicts handled?** The Add flow rejects a proposed id that
   exactly matches an existing status id for the selected host before opening a
   draft. Any filesystem-specific or race-condition conflict returned by the
   host remains a normal mutation error and must not discard the draft.
7. **What is the selection algorithm?** On status refresh for the same host,
   keep the selected id when it still exists; otherwise select the first status
   in server order. Server order is already bundled defaults first, then
   alphabetical for custom ids. If no status is available because loading failed
   or the directory cannot be read, show an empty/error state with no selection.
8. **How are reset/delete actions handled with unsaved edits?** If the action
   would discard the current draft or overwrite the currently selected file, the
   confirmation copy must cover both consequences. Actions against a different
   file do not discard the current draft unless the post-mutation selection
   change would.
9. **How are validation races handled?** Only validation results for the latest
   `(host, id, content)` tuple may affect the visible state or Save enablement.
   Save must perform or await validation for the exact content being written.
10. **How are error locations displayed?** `PipelineParseError.line` and
    `column` are already 1-based when present. The UI displays them directly
    and omits the location fragment when either value is absent.
11. **What does “raw TOML exactly” permit?** The editor must initialize from the
    raw endpoint string and write the user's current string unchanged. It may
    normalize only browser-controlled textarea behavior, not parse/reformat TOML
    or regenerate it from structured objects.
12. **Which translation files need keys?** Add English keys under the existing
    `settings` namespace and mirror the key structure in every existing locale
    file (`es`, `fr`, `ja`, `ko`, `zh-Hans`, `zh-Hant`) with English fallback
    strings where native translation is unavailable.
13. **Does this stage authorize backend work?** No. The current repository
    already exposes the needed routes, response types, id validation, raw read,
    write validation, bundled reset, reset-all, delete, status listing, and
    final-file default reseeding behavior. Later implementation should not
    change backend route shapes.
