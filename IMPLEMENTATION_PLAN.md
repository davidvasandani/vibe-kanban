# Implementation plan: default pipelines for new issues

Task `vk/16dc-default-pipeline`. See `SPEC.md` for the design and
`PRIOR_KNOWLEDGE.md` for the constraints this plan respects.

## Step 1: Config type (services)

- `crates/services/src/services/config/versions/v8.rs`: add
  `NewIssuePipelineDefaults { pipeline_ids, organizations, projects }` with
  `#[serde(default)]`. `Default` sets `pipeline_ids = ["basic"]`.
- Add `#[serde(default)] pub new_issue_pipelines` to `Config`, and to the
  `from_v7_config` and `Default` literals.
- `config/mod.rs`: re-export `pub type NewIssuePipelineDefaults`.
- Find every other `Config { .. }` struct literal (`rg "auto_error_remediation:"`)
  and add the field there too.
- Unit test: a v8 JSON without the field deserializes to the default.

## Step 2: Resolver (services::pipelines)

- `NewIssuePipelineSource` enum (`Project | Organization | Machine`, serde
  snake_case, TS).
- `NewIssuePipelineDefaultsResponse { pipeline_ids, source }` (TS).
- Pure `resolve_new_issue_pipelines(defaults, project_id, org_id)`, which
  returns `(Vec<String>, source)`.
- Pure `retain_available(ids, &[Pipeline])`: keeps configured order, drops
  missing ids, dedupes.
- Unit tests: precedence, an empty override means none, a missing org
  falls through to machine, filtering and ordering.

## Step 3: Route (server)

- `crates/server/src/routes/pipelines.rs`:
  `GET /pipelines/new-issue-defaults?project_id=`, with deployment state.
  - Read the config.
  - If there is no project override and `project_id` is set, look up the org
    via `deployment.remote_client()?.get_remote_project`, with a 5 s timeout
    and a warning on failure.
  - Resolve, then `retain_available(load_pipelines(..))`.
- `generate_types.rs`: register `NewIssuePipelineDefaults`,
  `NewIssuePipelineSource`, `NewIssuePipelineDefaultsResponse`. Run
  `pnpm run generate-types`.

## Step 4: MCP `create_issue`

- `remote_issues.rs`:
  - `pipeline_ids: None` → GET `/api/pipelines/new-issue-defaults?project_id=`.
    On error, warn and fall back to `[]`.
  - `Some(ids)` → unchanged (`Some([])` = none).
  - Resolve the pipeline objects from `/api/pipelines` in both cases.
    Defaulted ids are already filtered, so a missing one is skipped rather
    than raised as an error.
- Update the schemars descriptions for `pipeline_ids` and the
  `list_pipelines` tool text.
- Refactor the selection into a small pure helper so a unit test can cover
  "defaulted ids missing from catalog are skipped, explicit ones error".

## Step 5: Frontend data

- `api.ts`: `pipelinesApi.newIssueDefaults(projectId?)`.
- `usePipelines.ts`: `useNewIssuePipelineDefaults(projectId)` with query key
  `['pipelines', 'new-issue-defaults', projectId]`. It lives under
  `PIPELINES_QUERY_KEY`, so the existing invalidation also refreshes it.

## Step 6: `PipelineSection`

- `expanded` initial state is `false`.
- Collapsed header summary: the selected pipeline names after the title.
- A new `defaultPipelineIds?: string[] | null` prop:
  - `undefined` → legacy seeding (basic/first).
  - `null` → still loading, so wait.
  - an array → seed exactly those pipelines that exist, in their given
    order. The seed-once effect waits while it is `null`.
- Update `PipelineSection` tests/stories if present. Add a Vitest for the
  pure seeding helper (`seedPipelineIds(pipelines, defaults)`) in
  `taskPipeline.ts`.

## Step 7: Container

- `KanbanIssuePanelContainer`: call
  `useNewIssuePipelineDefaults(projectId)` (enabled in create mode only) and
  pass `defaultPipelineIds`: `data.pipeline_ids` on success, `[]` on error,
  `null` while loading.

## Step 8: Settings card

- New `NewIssuePipelineDefaultsCard.tsx`, rendered at the bottom of
  `PipelinesSettingsSection`:
  - Uses `useUserSystem()` (machine-scoped), `usePipelineStatuses` (valid
    ones), `useUserOrganizations` and `useAllOrganizationProjects`.
  - Machine checkboxes. Per org and per project, an inherit/override select
    plus checkboxes.
  - Each change calls `updateAndSaveConfig({ new_issue_pipelines: next })`
    with the whole object, then invalidates `PIPELINES_QUERY_KEY`.
  - Missing ids are shown with a "missing" badge and can be unticked.
- Pure helpers in `pipelineSettings.ts` (`setMachineDefault`,
  `setScopeOverride`, `clearScopeOverride`), with tests.
- i18n: new `settings.pipelines.newIssueDefaults.*` keys in every locale,
  plus a `taskPipeline` collapsed-summary key if one is needed.

## Step 9: Verify

- `cargo test -p services pipelines`, `cargo test -p mcp`,
  `cargo check --workspace`.
- `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`, the
  web-core vitest for the touched files, then `pnpm run format`.

## Step 10: Review, knowledge base, PR

- Codex review loop.
- Update `wiki/task-pipeline-block.md` (seeding defaults, collapse) and add
  a topic page on new-issue pipeline defaults. Update `wiki/INDEX.md`.
- Open the PR and merge it.
