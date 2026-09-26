//! Consumer for auto error remediation: turns each failed coding-agent turn
//! announced by `ContainerService::finalize_task` into a remote issue (with the
//! configured pipelines) and an unattended workspace linked to it.
//!
//! The guards and issue text live in `services::services::error_remediation`;
//! this module only resolves inputs and reuses the existing issue-creation,
//! workspace-start and link paths. A launch is attempted once: failures are
//! logged, never retried (VK constitution XLI).

use std::{collections::HashSet, sync::Arc, time::Instant};

use api_types::{CreateIssueRequest, IssuePriority, pipeline_block};
use axum::{Extension, Json, extract::State};
use db::models::{
    execution_process::ExecutionProcess,
    repo::Repo,
    requests::{CreateAndStartWorkspaceRequest, LinkedIssueInfo, WorkspaceRepoInput},
    session::Session,
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::profile::{
    ExecutorConfig, ExecutorConfigs, ExecutorProfileId, canonical_variant_key,
};
use services::services::{
    config::AutoErrorRemediationConfig,
    container::ContainerService,
    error_remediation::{
        ErrorRemediationEvent, ErrorRemediationGuard, GuardDecision, RemediationContext,
        compose_issue, extract_error_messages, remediation_stage_ids,
    },
    pipelines,
};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    routes::workspaces::{create::create_and_start_workspace, links},
};

/// Subscribe to failed-turn events and handle them in the background for the
/// lifetime of the process.
pub fn spawn(deployment: &DeploymentImpl) {
    let Some(sender) = deployment.container().error_remediation_sender() else {
        return;
    };
    let mut events = sender.subscribe();
    let deployment = deployment.clone();
    let guard = Arc::new(ErrorRemediationGuard::new());

    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    // Each event is handled on its own task so a slow remote
                    // call never delays the next event's guard decision.
                    let deployment = deployment.clone();
                    let guard = guard.clone();
                    tokio::spawn(async move { handle(&deployment, &guard, event).await });
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "Auto error remediation dropped failed-turn events");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn handle(
    deployment: &DeploymentImpl,
    guard: &ErrorRemediationGuard,
    ev: ErrorRemediationEvent,
) {
    let config = deployment
        .config()
        .read()
        .await
        .auto_error_remediation
        .clone();
    if !config.enabled {
        return;
    }

    let pool = &deployment.db().pool;
    let (workspace, session, execution) = match (
        Workspace::find_by_id(pool, ev.workspace_id).await,
        Session::find_by_id(pool, ev.session_id).await,
        ExecutionProcess::find_by_id(pool, ev.execution_process_id).await,
    ) {
        (Ok(Some(w)), Ok(Some(s)), Ok(Some(e))) => (w, s, e),
        _ => {
            tracing::warn!(
                workspace_id = %ev.workspace_id,
                execution_id = %ev.execution_process_id,
                "Auto error remediation skipped: could not load the failed turn"
            );
            return;
        }
    };

    match guard.try_reserve(
        workspace.id,
        workspace.name.as_deref(),
        Instant::now(),
        config.max_per_hour,
    ) {
        GuardDecision::Proceed => {}
        decision => {
            tracing::info!(
                workspace_id = %workspace.id,
                execution_id = %execution.id,
                ?decision,
                "Auto error remediation skipped"
            );
            return;
        }
    }

    match launch(deployment, &config, &workspace, &session, &execution).await {
        Ok((issue_id, launched_workspace_id)) => {
            guard.record_launched(launched_workspace_id);
            tracing::info!(
                source_workspace_id = %workspace.id,
                execution_id = %execution.id,
                %issue_id,
                %launched_workspace_id,
                "Auto error remediation launched"
            );
        }
        Err(error) => tracing::warn!(
            source_workspace_id = %workspace.id,
            execution_id = %execution.id,
            "Auto error remediation failed (not retried): {error}"
        ),
    }
}

/// Create the issue, start the workspace and link them. Returns the issue and
/// launched workspace ids.
async fn launch(
    deployment: &DeploymentImpl,
    config: &AutoErrorRemediationConfig,
    workspace: &Workspace,
    session: &Session,
    execution: &ExecutionProcess,
) -> Result<(Uuid, Uuid), String> {
    let pool = &deployment.db().pool;

    let project_id = match config.project_id {
        Some(project_id) => project_id,
        None => Workspace::get_remote_project_id(pool, workspace.id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("no project configured and the source workspace is not linked to one")?,
    };

    let repos = resolve_repos(deployment, config, workspace.id).await?;
    if repos.is_empty() {
        return Err("no repositories to start the remediation workspace on".to_string());
    }

    let error_messages = deployment
        .container()
        .normalized_entries(&execution.id)
        .await
        .map(|entries| extract_error_messages(&entries))
        .unwrap_or_default();

    let (variant, variant_fallback) = resolve_variant(config);

    let source_name = workspace
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| workspace.branch.clone());
    let draft = compose_issue(
        &RemediationContext {
            source_workspace_id: workspace.id,
            source_name,
            branch: workspace.branch.clone(),
            execution_process_id: execution.id,
            executor: session
                .executor
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            exit_code: execution.exit_code,
            error_messages,
            variant_fallback,
        },
        &compose_block(&config.pipeline_ids, &config.merge_stage_ids),
    );

    let client = deployment.remote_client().map_err(|e| e.to_string())?;
    let status_id = client
        .list_project_statuses(project_id)
        .await
        .map_err(|e| e.to_string())?
        .project_statuses
        .into_iter()
        .filter(|s| !s.hidden)
        .min_by_key(|s| s.sort_order)
        .map(|s| s.id)
        .ok_or("project has no visible statuses")?;

    let issue = client
        .create_issue(&CreateIssueRequest {
            id: None,
            project_id,
            status_id,
            title: draft.title.clone(),
            description: Some(draft.description.clone()),
            priority: Some(IssuePriority::High),
            start_date: None,
            target_date: None,
            completed_at: None,
            sort_order: 0.0,
            parent_issue_id: None,
            parent_issue_sort_order: None,
            extension_metadata: serde_json::json!({}),
        })
        .await
        .map_err(|e| e.to_string())?
        .data;

    let started = create_and_start_workspace(
        State(deployment.clone()),
        Json(CreateAndStartWorkspaceRequest {
            name: Some(draft.title.clone()),
            repos,
            linked_issue: Some(LinkedIssueInfo {
                remote_project_id: issue.project_id,
                issue_id: issue.id,
            }),
            executor_config: ExecutorConfig {
                executor: config.executor,
                variant,
                model_id: config.model_id.clone(),
                agent_id: None,
                reasoning_id: None,
                permission_policy: None,
            },
            prompt: format!("{}\n\n{}", draft.title, draft.description),
            attachment_ids: None,
            run_on_coordinator: false,
            requested_worker_node_id: None,
        }),
    )
    .await
    .map_err(|e| {
        format!(
            "issue {} created, but the workspace did not start: {e}",
            issue.id
        )
    })?
    .0
    .into_data()
    .ok_or_else(|| {
        format!(
            "issue {} created, but the workspace did not start",
            issue.id
        )
    })?
    .workspace;

    let launched_workspace_id = started.id;
    let _linked = links::link_workspace(
        Extension(started),
        State(deployment.clone()),
        Json(links::LinkWorkspaceRequest {
            project_id: issue.project_id,
            issue_id: issue.id,
        }),
    )
    .await
    .map_err(|e| {
        format!(
            "workspace {launched_workspace_id} started, but linking it to issue {} failed: {e}",
            issue.id
        )
    })?;

    Ok((issue.id, launched_workspace_id))
}

/// Configured repositories (each on the source workspace's target branch for
/// that repo when it has one, else the repo's default target branch, else
/// `main`), or the source workspace's own repositories when none are set.
async fn resolve_repos(
    deployment: &DeploymentImpl,
    config: &AutoErrorRemediationConfig,
    source_workspace_id: Uuid,
) -> Result<Vec<WorkspaceRepoInput>, String> {
    let pool = &deployment.db().pool;
    let source_repos = WorkspaceRepo::find_by_workspace_id(pool, source_workspace_id)
        .await
        .map_err(|e| e.to_string())?;

    if config.repo_ids.is_empty() {
        return Ok(source_repos
            .into_iter()
            .map(|r| WorkspaceRepoInput {
                repo_id: r.repo_id,
                target_branch: r.target_branch,
            })
            .collect());
    }

    let mut repos = Vec::with_capacity(config.repo_ids.len());
    let mut seen = HashSet::new();
    for repo_id in config
        .repo_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
    {
        let Some(repo) = Repo::find_by_id(pool, repo_id)
            .await
            .map_err(|e| e.to_string())?
        else {
            tracing::warn!(%repo_id, "Auto error remediation: configured repository not found");
            continue;
        };
        let target_branch = source_repos
            .iter()
            .find(|r| r.repo_id == repo_id)
            .map(|r| r.target_branch.clone())
            .or(repo.default_target_branch)
            .unwrap_or_else(|| "main".to_string());
        repos.push(WorkspaceRepoInput {
            repo_id,
            target_branch,
        });
    }
    Ok(repos)
}

/// The configured variant when it is defined for the executor; otherwise the
/// default variant, returning the missing name so the issue can say so.
fn resolve_variant(config: &AutoErrorRemediationConfig) -> (Option<String>, Option<String>) {
    let Some(variant) = config
        .variant
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(canonical_variant_key)
    else {
        return (None, None);
    };
    let profile = ExecutorProfileId {
        executor: config.executor,
        variant: Some(variant.clone()),
    };
    if ExecutorConfigs::get_cached()
        .get_coding_agent(&profile)
        .is_some()
    {
        (Some(variant), None)
    } else {
        (None, Some(variant))
    }
}

/// The pipeline block for the configured pipelines, composed with the shared
/// composer MCP `create_issue` and the New Issue UI use. Stages are each
/// pipeline's defaults, plus a merge stage when no default merges (see
/// `remediation_stage_ids`).
fn compose_block(pipeline_ids: &[String], merge_stage_ids: &[String]) -> String {
    let available = pipelines::load_pipelines(&utils::path::pipelines_dir());
    let selected: Vec<&pipelines::Pipeline> = pipeline_ids
        .iter()
        .filter_map(|id| {
            let found = available.iter().find(|p| &p.id == id);
            if found.is_none() {
                tracing::warn!(pipeline_id = %id, "Auto error remediation: unknown pipeline");
            }
            found
        })
        .collect();
    let enabled = remediation_stage_ids(&selected, merge_stage_ids);
    let block_pipelines: Vec<pipeline_block::BlockPipeline> =
        selected.into_iter().map(Into::into).collect();
    pipeline_block::compose_pipeline_block(&block_pipelines, &enabled, None)
}
