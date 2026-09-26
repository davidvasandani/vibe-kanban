//! Consumer for auto error remediation: turns each failed coding-agent turn
//! announced by `ContainerService::finalize_task` into a remote issue (with the
//! configured pipelines) and an unattended workspace linked to it.
//!
//! The guards and issue text live in `services::services::error_remediation`;
//! this module only resolves inputs and reuses the existing issue-creation,
//! workspace-start and link paths. A launch is attempted once: failures are
//! logged, never retried (VK constitution XLI).
//!
//! Before filing, it looks for an *active* remediation issue in the target
//! project whose recorded errors match this failure, and records the
//! occurrence there as a comment instead of starting parallel work on one root
//! cause. A lookup that cannot answer launches nothing.

use std::{collections::HashSet, sync::Arc, time::Instant};

use api_types::{
    CreateIssueCommentRequest, CreateIssueRequest, Issue, IssuePriority, IssueSortField,
    SearchIssuesRequest, SortDirection, pipeline_block,
};
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
        ErrorRemediationEvent, ErrorRemediationGuard, GuardDecision, REMEDIATION_MARKER_PREFIX,
        RemediationContext, compose_issue, compose_recurrence_comment, extract_error_messages,
        find_similar_issue, is_active_status_name, remediation_stage_ids,
    },
    pipelines,
    remote_client::RemoteClient,
};
use tokio::sync::{Mutex, broadcast::error::RecvError};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    routes::workspaces::{create::create_and_start_workspace, links},
};

/// Page size for the similar-issue lookup, newest first.
const SIMILAR_ISSUE_PAGE_SIZE: i32 = 200;
/// Pages read before the lookup gives up. Giving up fails closed (nothing is
/// launched): an unchecked remainder could hold the matching issue.
const SIMILAR_ISSUE_MAX_PAGES: usize = 10;

/// Shared state of the consumer.
#[derive(Default)]
struct Consumer {
    guard: ErrorRemediationGuard,
    /// Serializes "look for a similar issue → create one" so two failures
    /// with the same cause handled concurrently cannot both miss each other
    /// and file duplicates.
    lookup_lock: Mutex<()>,
}

enum LaunchOutcome {
    Launched { issue_id: Uuid, workspace_id: Uuid },
    Recurred { issue_id: Uuid },
}

/// Subscribe to failed-turn events and handle them in the background for the
/// lifetime of the process.
pub fn spawn(deployment: &DeploymentImpl) {
    let Some(sender) = deployment.container().error_remediation_sender() else {
        return;
    };
    let mut events = sender.subscribe();
    let deployment = deployment.clone();
    let consumer = Arc::new(Consumer::default());

    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    // Each event is handled on its own task so a slow remote
                    // call never delays the next event's guard decision.
                    let deployment = deployment.clone();
                    let consumer = consumer.clone();
                    tokio::spawn(async move { handle(&deployment, &consumer, event).await });
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "Auto error remediation dropped failed-turn events");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn handle(deployment: &DeploymentImpl, consumer: &Consumer, ev: ErrorRemediationEvent) {
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

    match consumer.guard.try_reserve(
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

    match launch(
        deployment,
        &consumer.lookup_lock,
        &config,
        &workspace,
        &session,
        &execution,
    )
    .await
    {
        Ok(LaunchOutcome::Launched {
            issue_id,
            workspace_id: launched_workspace_id,
        }) => {
            consumer.guard.record_launched(launched_workspace_id);
            tracing::info!(
                source_workspace_id = %workspace.id,
                execution_id = %execution.id,
                %issue_id,
                %launched_workspace_id,
                "Auto error remediation launched"
            );
        }
        Ok(LaunchOutcome::Recurred { issue_id }) => tracing::info!(
            source_workspace_id = %workspace.id,
            execution_id = %execution.id,
            %issue_id,
            "Auto error remediation recorded a recurrence on an active similar issue"
        ),
        Err(error) => tracing::warn!(
            source_workspace_id = %workspace.id,
            execution_id = %execution.id,
            "Auto error remediation failed (not retried): {error}"
        ),
    }
}

/// Reuse an active similar issue, or create the issue, start the workspace
/// and link them.
async fn launch(
    deployment: &DeploymentImpl,
    lookup_lock: &Mutex<()>,
    config: &AutoErrorRemediationConfig,
    workspace: &Workspace,
    session: &Session,
    execution: &ExecutionProcess,
) -> Result<LaunchOutcome, String> {
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
    let context = RemediationContext {
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
    };
    let draft = compose_issue(
        &context,
        &compose_block(&config.pipeline_ids, &config.merge_stage_ids)?,
    );

    let client = deployment.remote_client().map_err(|e| e.to_string())?;
    let statuses = client
        .list_project_statuses(project_id)
        .await
        .map_err(|e| e.to_string())?
        .project_statuses;
    let status_id = statuses
        .iter()
        .filter(|s| !s.hidden)
        .min_by_key(|s| s.sort_order)
        .map(|s| s.id)
        .ok_or("project has no visible statuses")?;
    let active_status_ids: Vec<Uuid> = statuses
        .iter()
        .filter(|s| is_active_status_name(&s.name))
        .map(|s| s.id)
        .collect();

    // Held until the new issue exists, so a concurrent failure with the same
    // cause finds it instead of filing a twin.
    let lookup = lookup_lock.lock().await;

    // Absent evidence never matches, and a project without active statuses
    // has no candidates (an empty `status_ids` filter is ambiguous).
    if !context.error_messages.is_empty() && !active_status_ids.is_empty() {
        let similar = find_active_similar_issue(
            &client,
            project_id,
            active_status_ids,
            &context.error_messages,
        )
        .await?;
        if let Some(existing) = similar {
            // The occurrence belongs to the existing issue either way; a
            // failed comment is logged, never a reason to launch.
            if let Err(error) = client
                .create_issue_comment(&CreateIssueCommentRequest {
                    id: None,
                    issue_id: existing.id,
                    message: compose_recurrence_comment(&context),
                    parent_id: None,
                })
                .await
            {
                tracing::warn!(
                    issue_id = %existing.id,
                    "Auto error remediation could not comment on the similar issue: {error}"
                );
            }
            return Ok(LaunchOutcome::Recurred {
                issue_id: existing.id,
            });
        }
    }

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
    // The issue is now findable; workspace start needs no serialization.
    drop(lookup);

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

    Ok(LaunchOutcome::Launched {
        issue_id: issue.id,
        workspace_id: launched_workspace_id,
    })
}

/// The newest active remediation issue in `project_id` whose recorded errors
/// match `messages`. Reads every page of candidates; an error, or a remainder
/// left unread after [`SIMILAR_ISSUE_MAX_PAGES`], is returned as `Err` so
/// the caller launches nothing rather than risk a duplicate.
async fn find_active_similar_issue(
    client: &RemoteClient,
    project_id: Uuid,
    active_status_ids: Vec<Uuid>,
    messages: &[String],
) -> Result<Option<Issue>, String> {
    let mut offset = 0usize;
    for _ in 0..SIMILAR_ISSUE_MAX_PAGES {
        let page = client
            .search_issues(&SearchIssuesRequest {
                project_id,
                status_id: None,
                status_ids: Some(active_status_ids.clone()),
                priority: None,
                parent_issue_id: None,
                search: Some(REMEDIATION_MARKER_PREFIX.trim().to_string()),
                simple_id: None,
                assignee_user_id: None,
                tag_id: None,
                tag_ids: None,
                sort_field: Some(IssueSortField::CreatedAt),
                sort_direction: Some(SortDirection::Desc),
                limit: Some(SIMILAR_ISSUE_PAGE_SIZE),
                offset: Some(offset as i32),
            })
            .await
            .map_err(|e| format!("similar-issue lookup failed, nothing launched: {e}"))?;
        if let Some(found) = find_similar_issue(&page.issues, messages) {
            return Ok(Some(found.clone()));
        }
        offset += page.issues.len();
        if page.issues.is_empty() || offset >= page.total_count {
            return Ok(None);
        }
    }
    Err(format!(
        "similar-issue lookup stopped after {offset} active remediation issues without \
checking them all; nothing launched"
    ))
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
///
/// Fails closed: a configured pipeline that is missing or does not parse
/// aborts the launch before any issue or workspace exists, rather than running
/// an unattended, merging agent without that pipeline's stages (e.g. review).
fn compose_block(pipeline_ids: &[String], merge_stage_ids: &[String]) -> Result<String, String> {
    let available = pipelines::load_pipelines(&utils::path::pipelines_dir());
    let selected = select_pipelines(&available, pipeline_ids)?;
    let enabled = remediation_stage_ids(&selected, merge_stage_ids);
    let block_pipelines: Vec<pipeline_block::BlockPipeline> =
        selected.into_iter().map(Into::into).collect();
    Ok(pipeline_block::compose_pipeline_block(
        &block_pipelines,
        &enabled,
        None,
    ))
}

/// The configured pipelines in configured order, or an error naming every id
/// that is not among the loaded (valid) pipelines.
fn select_pipelines<'a>(
    available: &'a [pipelines::Pipeline],
    pipeline_ids: &[String],
) -> Result<Vec<&'a pipelines::Pipeline>, String> {
    let mut selected = Vec::with_capacity(pipeline_ids.len());
    let mut missing = Vec::new();
    for id in pipeline_ids {
        match available.iter().find(|p| &p.id == id) {
            Some(pipeline) => selected.push(pipeline),
            None => missing.push(id.as_str()),
        }
    }
    if missing.is_empty() {
        Ok(selected)
    } else {
        Err(format!(
            "configured pipeline(s) missing or invalid: {}",
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use services::services::pipelines::{Pipeline, PipelineStep};

    use super::*;

    fn pipeline(id: &str) -> Pipeline {
        Pipeline {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            stages: vec![PipelineStep {
                id: "spec".to_string(),
                label: "spec".to_string(),
                prompt_fragment: "Write a spec.".to_string(),
                default_enabled: true,
                heavy: false,
            }],
        }
    }

    #[test]
    fn selects_configured_pipelines_in_configured_order() {
        let available = vec![pipeline("speckit"), pipeline("wikillm")];
        let ids = vec!["wikillm".to_string(), "speckit".to_string()];
        let selected = select_pipelines(&available, &ids).unwrap();
        let order: Vec<&str> = selected.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(order, vec!["wikillm", "speckit"]);
    }

    #[test]
    fn a_missing_pipeline_fails_closed() {
        let available = vec![pipeline("speckit")];
        let ids = vec!["wikillm".to_string(), "speckit".to_string()];
        let error = select_pipelines(&available, &ids).unwrap_err();
        assert!(error.contains("wikillm"));
    }
}
