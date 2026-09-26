# Implementation Plan — Auto Error Remediation (`vk/7e4f-auto-error-remed`)

See `SPEC.md` for the design and `PRIOR_KNOWLEDGE.md` for the constraints.

## Step 1: Shared pipeline-block composer (`crates/api-types`)

1. Add `src/pipeline_block.rs`, exported from `lib.rs`. It contains:
   - `BlockStage { id, prompt_fragment }` and `BlockPipeline { name, stages }`;
   - `canonical_stage_order`, `compose_executor_line`,
     `compose_pipeline_block` and `append_pipeline_block`, moved verbatim
     from `crates/mcp/src/task_server/tools/pipelines.rs` and re-typed onto
     the new structs.
2. Move the composer unit tests along with the code.
3. In `crates/mcp`, keep `McpPipeline`/`McpPipelineStep` for the tool
   schema. Add `impl From<&McpPipeline> for BlockPipeline`, delete the moved
   functions, and point `remote_issues.rs` at `api_types::pipeline_block`.
4. Add `impl From<&Pipeline> for BlockPipeline` in `services::pipelines`.

## Step 2: Config (`crates/services/src/services/config/versions/v8.rs`)

1. Add the `AutoErrorRemediationConfig` struct with `Default` using the SPEC
   defaults (`CLAUDE_CODE`, `Some("PROALIGN")`, `Some("claude-opus-5-5")`,
   `["wikillm","speckit"]`, 3/h, disabled). Derive the TS type.
2. Add `#[serde(default)] pub auto_error_remediation` to `Config`, and set it
   to its default in both the `from_v7_config` and `Default` constructors.
3. Add the new type to `crates/server/src/bin/generate_types.rs` and run
   `pnpm run generate-types`.

## Step 3: Trigger (`crates/services`)

1. Add a new module `services/error_remediation.rs` containing:
   - `ErrorRemediationEvent { workspace_id, session_id, execution_process_id }`;
   - `fn is_remediation_trigger(run_reason, status) -> bool`, which is pure;
   - `AUTO_REMEDIATION_NAME_PREFIX = "Auto-fix: "`;
   - `ErrorRemediationGuard`: an in-memory, `Mutex`-wrapped guard with
     `check(workspace_name, workspace_id, now, max_per_hour) -> Decision` and
     `record(source_ws, launched_ws, now)`;
   - `fn compose_issue(ctx) -> (title, description)`, a pure body builder
     that uses the marker plus `pipeline_block`;
   - unit tests for all of the above.
2. Add a trait method on `ContainerService`:
   `fn error_remediation_sender(&self) -> Option<&broadcast::Sender<ErrorRemediationEvent>> { None }`.
3. In `finalize_task`, after the notification, when
   `is_remediation_trigger(..)` holds and
   `self.config().read().await.auto_error_remediation.enabled`, call
   `sender.send(ev)` and ignore any error. Check whether the trait exposes
   `config()`; if it doesn't, read the config in the local impl.
4. In `crates/local-deployment`, give `LocalContainerService` a
   `broadcast::Sender` field (capacity 64), created in `new()`, and override
   the trait method.

## Step 4: Launcher (`crates/server/src/error_remediation.rs`)

1. `pub fn spawn(deployment)` subscribes to the sender and spawns a loop.
   On `Lagged` it logs and continues; on `Closed` it exits.
2. `handle(deployment, guard, ev)` does the following, in order:
   1. Load the workspace, session and execution; skip if any is missing.
   2. Read the config snapshot and return if it is disabled.
   3. Run `guard.check`.
   4. Resolve the project: the configured `project_id`, else
      `Workspace::get_remote_project_id`, else skip with `info`.
   5. Resolve repos: configured `repo_ids` mapped to
      `Repo.default_target_branch`, falling back to the source workspace's
      target branch for that repo and then to `main`. If none are
      configured, use the source `WorkspaceRepo`s.
   6. Collect error entries with `container.normalized_entries`: keep the
      last 5 `ErrorMessage` entries, each capped at 2 000 chars.
   7. Load the pipelines with `services::pipelines::load_pipelines(dir)`,
      using the same dir the `/api/pipelines` route uses, and select them by
      id. Compose the block from the default-enabled stages.
   8. Resolve the variant with `ExecutorConfigs::get_cached().get_coding_agent(..)`.
      If it is missing, fall back to `None` and add a note to the issue.
   9. Pick the status: the first visible entry from
      `remote_client.list_project_statuses`, ordered by `sort_order`.
   10. Create the issue with `remote_client.create_issue(CreateIssueRequest{..priority: High..})`.
   11. Start the workspace by calling
       `routes::workspaces::create::create_and_start_workspace(State, Json(req))`.
   12. Link it by calling `routes::workspaces::links::link_workspace(Extension(ws), State, Json(..))`.
   13. Run `guard.record`.
3. Call `error_remediation::spawn` from `startup.rs` after the container is
   ready.

## Step 5: Frontend

1. Add `AutoErrorRemediationSettingsCard.tsx` in the settings folder and
   render it in `GeneralSettingsSection` after Task Execution. It contains:
   - an enable checkbox with a warning;
   - a project-id input;
   - repo checkboxes (from the existing repos query);
   - an executor select;
   - a variant input;
   - a model input;
   - a max-per-hour input.
2. Add i18n keys under `settings.general.autoRemediation.*` for all 7
   locales, with English copy in each as the precedent does.
3. Add a lightweight Vitest test for the card: the toggle updates the draft.

## Step 6: Verify

- `cargo test -p api-types -p services -p mcp -p server`
- `pnpm run generate-types:check`
- `pnpm run check`
- `pnpm run lint`
- `pnpm run format`

## Step 7: Wiki

- Add a new page, `wiki/auto-error-remediation.md`, and update `INDEX.md`.
- Update `task-pipeline-block.md` to note that the Rust composer now lives in
  `api-types`.
