# Prior knowledge: `vk/7e4f-auto-error-remed`

This file distills the knowledge-base pages (`wiki/`) that bear on
auto-remediating failed agent turns. It is read-only input to the spec and
plan stages.

## task-pipeline-block.md

- The pipeline block (`<!-- vk:pipeline:start/end -->`) inside
  `issues.description` is the **only** record of a pipeline selection. No
  structured copy exists. "Enabling pipelines on an issue" therefore means
  composing that block into the description.
- The block has one Rust composer (MCP `compose_pipeline_block`) and one TS
  composer (`taskPipeline.ts`), and they must stay byte-identical. The edit UI
  round-trips a block only when it matches the composer exactly. A
  hand-rolled third composer could produce a block the UI misparses; if the
  UI under-recognizes a selection, recomposing drops stage lines. The auto
  path must therefore **reuse** the existing Rust composer, not re-implement
  it.
- Display names aren't unique across pipeline TOML files, so select pipelines
  by **id** (`wikillm`, `speckit`).

## issue-workspace-lifecycle.md

- Remote (Postgres) and local (SQLite) stores sync on a best-effort basis:
  errors are logged, and there is no distributed transaction or durable retry
  queue. An auto-launcher that creates a remote issue and then a local
  workspace inherits that model. It should log partial failure rather than
  retry blindly.
- Completion-time code must read **current** workspace metadata, not a
  snapshot captured earlier.

## workspace-creation-reliability.md

- `create_and_start_workspace` accepts the request and then runs creation in
  a spawned task (queue → claim → run → finish). The returned workspace is
  the accepted record, and creation can still fail afterwards. Callers
  (MCP `start_workspace`, and now the launcher) link the issue after the
  accepted response, which matches the MCP flow.
- Never retry the workspace workflow on failure. The same rule applies to
  remediation: a failed launch is logged, not retried.

## agent-process-lifecycle.md / vk-pollers.md

- One turn is one `ExecutionProcess`. `Indeterminate` means the terminal
  event was lost and the group was reaped, so it is **not** evidence of an
  agent error. `Killed` means the user stopped the turn, and `Interrupted`
  means a server restart stopped it.
- `CodingAgent` is non-persistent, so every turn goes through finalization.
  `finalize_task` is the single place where every exit path (local exit
  monitor, skipped cleanup, worker reconciliation) sends the
  completion/failure notification. That makes it the natural trigger point.

## Precedent (git history, not wiki)

- `resume_interrupted_on_startup` (#62) is the closest analogue: an opt-in
  config flag that spawns agents without a human. Three things to copy:
  - it is off by default, with `#[serde(default)]` on Config v8;
  - it has a General-settings toggle with i18n in every locale;
  - it caps itself so a crash loop cannot keep respawning agents.

## Gaps

- The wiki has no page on unattended or auto-spawned workspaces, or on
  loop-guarding. This task should add one (enrich stage).
