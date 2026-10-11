# SPEC: Default pipelines for new issues

Task: `vk/16dc-default-pipeline`. Feature spec, plan and tasks:
`specs/vk/16dc-default-pipeline/` (homelab repo).

## Problem

1. The **Pipeline** card in the kanban issue panel (create and edit modes)
   renders expanded. It shows the pipeline checkboxes, every stage checkbox,
   and a 6-row textarea. That pushes the rest of the panel (create button,
   SpecKit, relationships, comments) below the fold on every issue.
2. Which pipelines a new issue starts with is hard-coded and inconsistent:
   - The New Issue UI always pre-selects the `basic` pipeline (or the first
     pipeline when `basic` is absent).
   - MCP `create_issue` attaches **no** pipeline unless the caller passes
     `pipeline_ids`.
   No setting lets an operator say "new issues get WikiLLM + SpecKit", and
   nothing lets one project or organization differ from the rest.

## Goals

- G1. The Pipeline card starts **collapsed** in both create and edit modes.
  While collapsed, its header shows which pipelines are selected, so a
  collapsed card never hides an attached pipeline.
- G2. A per-machine setting lists the pipelines that start enabled on new
  issues. Each pipeline can be turned on or off individually.
- G3. Organizations and projects can override that setting. A project
  override beats an organization override, which beats the machine setting.
  An override can be "no pipelines".
- G4. The resolved defaults apply to every new issue, whether it is created
  in the New Issue UI or through MCP `create_issue`. An explicit choice at
  issue creation still wins: ticking/unticking in the UI, or passing
  `pipeline_ids` (including `[]`) to MCP.

## Non-goals

- Auto error remediation keeps its own `pipeline_ids` setting. It always
  names its pipelines explicitly, so the new defaults do not touch it.
- Issues created by Jira or Slack inbound sync and the remote-web frontend
  are out of scope. The defaults live in the local host's config, which
  those paths do not read.
- No per-stage defaults beyond what already exists. Stages still start
  ticked according to each pipeline's `default_enabled` flags.
- No cloud (remote Postgres) schema change. See "Storage" below.

## Design

### Storage: machine config, not the cloud database

Pipelines themselves are per-machine TOML files (`~/.vibe-kanban/pipelines/`),
and both the UI picker and MCP read them from the local backend
(`GET /api/pipelines`). Pipeline ids only mean something on the machine that
defines them, so the defaults and their org/project overrides live beside
them, in the machine's `config.json`:

```rust
/// Pipelines enabled by default on new issues (UI and MCP `create_issue`).
#[serde(default)]
pub struct NewIssuePipelineDefaults {
    /// Machine-wide default. Defaults to ["basic"], the old UI behavior.
    pub pipeline_ids: Vec<String>,
    /// Organization id → pipeline ids. A present key overrides the machine
    /// default; an empty list means "no pipelines".
    pub organizations: HashMap<Uuid, Vec<String>>,
    /// Project id → pipeline ids. Beats the organization override.
    pub projects: HashMap<Uuid, Vec<String>>,
}

Config.new_issue_pipelines: NewIssuePipelineDefaults   // #[serde(default)]
```

`#[serde(default)]` on the new field means existing v8 configs load
unchanged, so no config version bump is needed.

Putting the overrides in the cloud `projects`/`organizations` tables was
rejected. It needs a remote migration plus a regenerated SQLx offline cache,
and it would sync pipeline ids to machines that may not define them.

### Resolution: one server endpoint

`GET /api/pipelines/new-issue-defaults?project_id=<uuid>` returns:

```json
{ "pipeline_ids": ["wikillm", "speckit"], "source": "project" }
```

`source` is one of `project`, `organization` or `machine`. Resolution order:

1. `projects[project_id]` if present.
2. Otherwise `organizations[org_id]` if present. `org_id` comes from
   `remote_client.get_remote_project(project_id)`, bounded by a 5 s timeout.
   If that lookup fails, skip this step and log a warning.
3. Otherwise the machine `pipeline_ids`.

Without `project_id`, only step 3 applies. The resolved ids are then
filtered to pipelines that currently load (`load_pipelines`), keeping the
configured order. A deleted or broken pipeline is dropped quietly instead of
failing issue creation. The pure resolver
(`resolve_new_issue_pipelines(config, project_id, org_id)`) and the filter
are unit-tested in `services::services::pipelines`.

### UI: create mode

- `KanbanIssuePanelContainer` passes the project id to the create-mode
  `PipelineSection`. A new hook, `useNewIssuePipelineDefaults(projectId)`,
  calls the endpoint.
- `PipelineSection` gets a `defaultPipelineIds?: string[]` prop that
  replaces the hard-coded `basic` seeding. Seeding still happens exactly
  once, after both the pipeline list and the defaults have settled. If the
  defaults query fails, it seeds nothing rather than guessing. Stage ticks
  are the union of the seeded pipelines' `default_enabled` stages, as before.
- The operator's ticks and unticks before submit are the issue-creation
  override. Nothing else changes in submit.

### UI: collapsed by default

- `PipelineSection` initialises `expanded` to `false` in both modes.
- When collapsed and at least one pipeline is selected, the header shows the
  selected names after the title, for example `Pipeline · WikiLLM, SpecKit`.
  With none selected it shows the title only.
- Collapsing only hides the card. The composed block is still emitted and
  appended on submit exactly as when it is expanded.

### UI: Settings → Pipelines → "New issue defaults" card

This card sits in the machine-scoped Pipelines section, beneath the file
editor. It saves through the machine-scoped `useUserSystem`
(`SettingsMachineUserSystemProvider`) and replaces the whole
`new_issue_pipelines` object on every change, because `updateDraft` merges
arrays by index (see wiki `auto-error-remediation`). It has three parts:

- **Machine default**: one checkbox per valid pipeline ("enabled by default
  on new issues").
- **Organization overrides**: per organization, a select with "Use machine
  default" or "Override". Choosing Override reveals per-pipeline checkboxes.
  Override with nothing ticked means "no pipelines".
- **Project overrides**: the same control per project, grouped under
  `useAllOrganizationProjects`. Its inherit label says "Use organization
  default".

Each change saves immediately with `updateAndSaveConfig`, like the other
pipeline mutations in this section, and the `new-issue-defaults` query is
invalidated afterwards. Override ids that no longer exist as pipeline files
are shown as "missing" and can be cleared.

### MCP `create_issue`

- `pipeline_ids` omitted (`None`): call
  `/api/pipelines/new-issue-defaults?project_id=…` and use the result. If
  the call fails, create the issue without a pipeline and log a warning.
  Issue creation must not fail because defaults could not be resolved.
- `pipeline_ids: []`: explicitly no pipeline. This is the issue-creation
  override.
- `pipeline_ids: [...]`: unchanged (unknown id → error).
- `pipeline_stage_ids` keeps its meaning across whichever pipelines were
  selected.
- The tool and parameter descriptions are updated to explain the defaults
  and the `[]` opt-out.

## Acceptance criteria

1. Opening New Issue or an existing issue shows the Pipeline card collapsed.
   Its header names the selected pipelines.
2. With a fresh config, New Issue pre-selects `basic`, matching today, and
   MCP `create_issue` without `pipeline_ids` attaches the `basic` block with
   its default stages.
3. Setting the machine default to `{wikillm, speckit}` makes New Issue
   pre-select both, and makes MCP `create_issue` without `pipeline_ids`
   produce the same block the UI would.
4. A project override beats an organization override, which beats the
   machine default. An override with nothing ticked yields no pipeline.
5. MCP `create_issue` with `pipeline_ids: []` yields no pipeline regardless
   of the defaults.
6. Default ids naming a missing pipeline are skipped. They never block
   issue creation.
7. `pnpm run check`, `pnpm run lint`, `cargo test --workspace` (the touched
   crates), and `pnpm run generate-types:check` all pass.
