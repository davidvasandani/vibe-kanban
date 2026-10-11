# New-issue pipeline defaults: one resolver, per-machine storage

Which pipelines a new issue starts with is decided by
`Config.new_issue_pipelines` (`NewIssuePipelineDefaults`). The Pipelines
settings page edits it in the **New issue defaults** card. Before
`vk/16dc-default-pipeline` the New Issue UI hard-coded `basic` and MCP
`create_issue` attached nothing, so the two paths disagreed. This page
records why the design looks the way it does (constitution LI).

## Storage: machine config, not cloud columns

- Pipelines are per-machine TOML files, so a pipeline id only means
  something on the machine that defines it. The machine default and the
  org/project overrides therefore live in the same machine `config.json`:
  - `pipeline_ids`, defaulting to `["basic"]`, the old UI behaviour.
  - `organizations`, a map of org UUID to ids.
  - `projects`, a map of project UUID to ids.
  `#[serde(default)]` means no config version bump.
- Rejected alternatives:
  - Columns on the cloud `projects`/`organizations` tables. These need a
    remote migration plus a regenerated SQLx offline cache, which needs
    Postgres. They would also sync ids to machines that lack those
    pipelines.
  - A `default_on_new_issues` key in each TOML. Toggling it would rewrite
    user TOML and lose comments, "Reset default" would wipe it, and it
    can't express overrides.

## Resolution rules

- `services::pipelines::resolve_new_issue_pipelines(defaults, project, org)`
  is pure and resolves in order: project override, then org override, then
  machine default.
  - A **present key is authoritative even when empty**: `[]` means none.
  - Only an absent key inherits.
  - In the UI, "Use … default" deletes the key, and "Override" with
    nothing ticked stores `[]`.
- `retain_available` then drops ids whose pipeline no longer loads,
  dedupes, and keeps the configured order. A deleted pipeline never
  blocks issue creation.
- `GET /api/pipelines/new-issue-defaults?project_id=` is the single entry
  point for both the UI and MCP.
  - The project → org lookup (`get_remote_project`, 5 s timeout) runs only
    when the project has no override of its own.
  - If the lookup fails, the org level is skipped and a warning is logged.
    The endpoint never errors.

## MCP `create_issue`

- Omitted `pipeline_ids` → the resolved defaults. If resolution fails, the
  issue is created without a pipeline and a warning is logged.
- `[]` → explicitly none. This is the issue-creation override.
- Explicit ids → unchanged. An unknown id is still an error.
- `select_pipelines(catalog, ids, explicit)` encodes this asymmetry: a
  missing explicit id errors, a missing defaulted id is skipped.

## UI gotchas (all three found by Codex review)

- **Seeding waits, and nothing runs ahead of it.**
  - `PipelineSection`'s `defaultPipelineIds` prop: `null` means still
    loading, so the seed-once effect waits; `undefined` means legacy
    `basic`/first.
  - While unseeded, the picker's controls are disabled, because a tick
    made before the defaults arrive would be overwritten by the seed.
  - The container blocks Create (`isSubmitBlocked` on `KanbanIssuePanel`,
    plus a guard in `handleSubmit`) while the pipeline list or defaults
    are pending. Otherwise a fast submit during the up-to-5 s org lookup
    silently creates the issue without its pipelines.
- **Pipeline queries are host-scoped.**
  - The keys are `['pipelines', <host scope>]` and
    `['pipelines', 'new-issue-defaults', <host scope>, projectId]`, and
    requests go through `makeHostAwareRequest`.
  - A project-only key let a host switch reuse the other machine's
    defaults for 60 s.
  - The keys stay under the `['pipelines']` prefix, so existing
    invalidation still covers them.
- **The settings card saves the whole object.** Each change calls
  `updateAndSaveConfig({ new_issue_pipelines: next })` with a fully rebuilt
  object, because a deep merge can't shrink arrays or delete map keys (the
  same trap as [[auto-error-remediation]]).
  - The pure updaters live in `lib/pipeline/pipelineSettings.ts`.
  - `togglePipelineId` keeps catalog order, so click order never changes
    the composed block.

## Collapsed by default

The Pipeline card starts collapsed in both modes. The header button carries
`aria-expanded` and appends `· Name, Name` for the selected pipelines (XXVI:
collapsed controls keep decisive context). The card stays mounted while
collapsed because it owns the selection and emits the block on submit. See
[[task-pipeline-block]].

## Contributed by
- vk/16dc-default-pipeline
