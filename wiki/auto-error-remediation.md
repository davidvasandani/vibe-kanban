# Auto error remediation: self-spawned agent work that cannot run away

When a coding-agent turn fails, VK can file an issue that carries the
configured pipelines and start an unattended workspace on it
(`Config.auto_error_remediation`, off by default). This page records the
design choices and gotchas that apply to any feature that spawns agents
without a human (VK constitution XLI).

## Trigger at `finalize_task`, hand off through a channel

`ContainerService::finalize_task` is the one place every exit path converges.
That includes the local exit monitor, the skipped-cleanup path, and worker
reconciliation (see [[agent-process-lifecycle]]). It already filters out
`Killed` (the user stopped the turn), `Interrupted` (a restart stopped it) and
`Indeterminate` (terminal evidence was lost), none of which is an agent
error. The trigger is `run_reason == CodingAgent && status == Failed`, and
nothing else.

`services` cannot call server route handlers, and finalization must not block
on remote HTTP. So `finalize_task` only does a non-blocking `broadcast::send`,
through the trait method `error_remediation_sender()`, which defaults to
`None`. `server::error_remediation` subscribes and does the I/O.
`ContainerService` has **no config accessor**, so the event is emitted
unconditionally and the consumer checks `enabled`. That also means toggling
the setting takes effect without a restart.

**Gotcha: subscribe before anything can fail.** A broadcast sent with no
receiver is lost. The consumer must subscribe immediately after
`DeploymentImpl::new`, before boot resumes interrupted coding agents
(`resume_interrupted_on_startup`); otherwise an early resumed failure is
dropped. This was Codex review round 1.

## Guards: reserve before I/O, fail closed

`ErrorRemediationGuard::try_reserve` makes its decision and claims the slot
atomically, before any remote call:

- **Recursion.** A source whose name starts with `Auto-fix: `, or that this
  process launched, never triggers. The name prefix is the *durable* guard
  that survives restarts. The in-memory set only covers renames.
- **Dedupe.** Each source workspace is remediated at most once per 24 h.
- **Cap.** At most `max_per_hour` launches in the trailing hour, and `0`
  launches nothing. A rate-limited attempt does *not* consume the source's
  dedupe slot.

Reserving a slot before I/O means a launch that fails half-way still counts,
and it is never retried. Recording only after success would let a
created-issue/failed-workspace sequence file duplicates. Dedupe and cap state
are in memory, so the worst case after a restart is `max_per_hour` extra
launches. Recursion is still blocked by the name prefix.

A configured pipeline that is missing or does not parse aborts the launch
before anything is created. Dropping it silently would run an unattended,
merging agent without, for example, its review stage (Codex round 1).

## Reuse the existing paths; call handlers directly

- **Issue.** `RemoteClient::create_issue`. The status is the first visible
  project status by `sort_order`, the same rule as MCP `default_status_id`.
- **Workspace.** Call `routes::workspaces::create::create_and_start_workspace`
  and `links::link_workspace` directly. Axum extractors (`State`, `Json`,
  `Extension`) are plain tuple structs. This inherits queueing, placement,
  attachment import and project-context composition
  ([[workspace-creation-reliability]]). The prompt is `title + description`,
  exactly as MCP `start_workspace` builds it from an issue. That means
  `is_speckit_pipeline(prompt)` provisions SpecKit automatically.
- **Pipeline block.** The Rust composer now lives in
  `api_types::pipeline_block` and is shared by MCP `create_issue` and the
  remediation consumer ([[task-pipeline-block]]).

## Gotcha: bundled pipelines differ from deployed ones

The live `~/.vibe-kanban/pipelines/*.toml` files are user-editable and have
drifted from `assets/pipelines/`:

- This deployment's SpecKit pipeline has a default-on `pr-and-merge` stage.
- The bundled SpecKit pipeline has only a default-off `merge` stage.

"Default stages" therefore do not reliably include a merge. An unattended fix
is only useful once it merges, so `remediation_stage_ids` adds the first
existing id from `merge_stage_ids` (default `["pr-and-merge", "merge"]`) when
no default stage already merges. When one does, the selection is exactly the
UI default and the block round-trips. A test that pinned the block
byte-for-byte to a real issue's block failed against the bundled assets.
Never test pipeline selection against deployed-pipeline text.

## Settings UI gotcha: `updateDraft` deep-merges arrays

`GeneralSettingsSection.updateDraft` uses lodash `merge`, which merges arrays
**by index**. `merge([a, b], [b])` yields `[b, b]`, not `[b]`, so deselecting a
repo can never shrink the list. The remediation card therefore replaces the
whole `auto_error_remediation` object through a dedicated setter. Use the same
approach for any array-valued config edited through that section.

## Contributed by

- `vk/7e4f-auto-error-remed`
