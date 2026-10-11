# Prior knowledge: default pipelines for new issues

Task `vk/16dc-default-pipeline`. Distilled from the project knowledge base
(`wiki/`), which I only read. Relevant pages:
`task-pipeline-block`, `kanban-issue-panel-sections`, `auto-error-remediation`,
`bundled-file-seed-manifests`, `agent-picker-visibility`, `frontend-linting`
and `create-mode-repo-branch-defaulting`.

## Pipeline block (wiki: task-pipeline-block)

- The issue description is the only place the selection is stored, as a
  delimited `## Pipeline` block. There is no structured copy, and adding one
  would create a second source of truth. This task changes only which
  pipelines are *seeded*, not where they are stored.
- `PipelineSection` is uncontrolled and seeded exactly once, after the
  pipeline list loads. Callers reseed by remounting it via `key`.
  - Don't reseed ticks from an effect that watches the selection, because it
    clobbers the user's ticks.
  - New seeding inputs (the resolved defaults) must join the single
    seed-once effect. Seeding must wait until *both* the pipeline list and
    the defaults have settled, or it will seed from a half-loaded state.
- Pipeline toggles adjust ticks incrementally. Each seeded pipeline enables
  its own `default_enabled` stages.
- Rust composer: every backend path that writes a block, MCP `create_issue`
  included, must go through `api_types::pipeline_block`, so its output stays
  byte-identical to the TS composer. MCP's defaulted path must reuse it.

## Issue panel layout (wiki: kanban-issue-panel-sections)

- `renderPipeline` renders in both modes. The container branches per mode:
  create stashes the block until submit; edit seeds from the stored block
  and applies it with Update Issue. Collapsing is internal to
  `PipelineSection`, so neither path changes.
- The panel is shared by local-web and remote-web.

## Settings (wiki: auto-error-remediation, agent-picker-visibility)

- `GeneralSettingsSection.updateDraft` uses lodash `merge`, which merges
  arrays **by index**, so deselecting an item can never shrink a list.
  Replace whole objects that contain arrays (the remediation card does this
  for `auto_error_remediation`).
- Settings under "Machine settings" are per-host. `useUserSystem` inside the
  settings dialog is wrapped by `SettingsMachineUserSystemProvider`, so
  `config` and `updateAndSaveConfig` target the selected machine.
- Bundled and deployed pipelines differ: the live TOMLs are user-editable.
  Never test against deployed pipeline text, and never assume a pipeline id
  exists. Defaults must tolerate missing ids.
- Auto error remediation already has its own `pipeline_ids`. It must keep
  naming its pipelines explicitly and must not inherit the new defaults.

## Frontend checks (wiki: frontend-linting)

- ESLint forbids directive comments outright.
- The i18n check requires key parity across every locale (en, es, fr, ja,
  ko, zh-*). Every new key must be added to all of them.
- `pnpm run lint` also runs the unused-i18n-key check.

## Config storage precedent

- Fields added to the v8 `Config` use `#[serde(default)]`, so old configs
  load without a version bump (examples: `resume_interrupted_on_startup`,
  `auto_error_remediation`).
- The precedent for remote project fields is project `context`
  (`vk/4931-project-context-ui`). That needs a remote migration and a
  regenerated SQLx offline cache, which requires Postgres, and this
  environment has none. That is one reason to store overrides locally.
