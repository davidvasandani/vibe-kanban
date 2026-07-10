use axum::{
    Extension, Json, Router,
    extract::State,
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    session::{CreateSession, Session},
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::actions::{
    ExecutorAction, ExecutorActionType,
    script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum RunScriptError {
    NoScriptConfigured,
    ProcessAlreadyRunning,
}

/// Cap on concurrently running background helpers per workspace, so a
/// misbehaving agent cannot accumulate an unbounded process fleet.
const MAX_BACKGROUND_HELPERS_PER_WORKSPACE: usize = 5;

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum StartBackgroundHelperError {
    EmptyScript,
    InvalidWorkingDir,
    TooManyHelpers,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct StartBackgroundHelperRequest {
    /// Bash script to run as the helper (e.g. a watcher or tunnel).
    pub script: String,
    /// Optional path to run the script in, relative to the workspace root.
    #[serde(default)]
    pub working_dir: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/dev-server/start", post(start_dev_server))
        .route("/background-helpers", get(list_background_helpers))
        .route("/background-helpers/start", post(start_background_helper))
        .route("/cleanup", post(run_cleanup_script))
        .route("/archive", post(run_archive_script))
        .route("/stop", post(stop_workspace_execution))
}

#[axum::debug_handler]
pub async fn start_dev_server(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ExecutionProcess>>>, ApiError> {
    let pool = &deployment.db().pool;

    let existing_dev_servers =
        match ExecutionProcess::find_running_dev_servers_by_workspace(pool, workspace.id).await {
            Ok(servers) => servers,
            Err(e) => {
                tracing::error!(
                    "Failed to find running dev servers for workspace {}: {}",
                    workspace.id,
                    e
                );
                return Err(ApiError::Workspace(
                    db::models::workspace::WorkspaceError::ValidationError(e.to_string()),
                ));
            }
        };

    for dev_server in existing_dev_servers {
        tracing::info!(
            "Stopping existing dev server {} for workspace {}",
            dev_server.id,
            workspace.id
        );

        if let Err(e) = deployment
            .container()
            .stop_execution(&dev_server, ExecutionProcessStatus::Killed)
            .await
        {
            tracing::error!("Failed to stop dev server {}: {}", dev_server.id, e);
        }
    }

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let repos_with_dev_script: Vec<_> = repos
        .iter()
        .filter(|r| r.dev_server_script.as_ref().is_some_and(|s| !s.is_empty()))
        .collect();

    if repos_with_dev_script.is_empty() {
        return Ok(ResponseJson(ApiResponse::error(
            "No dev server script configured for any repository in this workspace",
        )));
    }

    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: Some("dev-server".to_string()),
                    name: None,
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let mut execution_processes = Vec::new();
    for repo in repos_with_dev_script {
        // Guaranteed `Some` by the `is_some_and(|s| !s.is_empty())` filter above,
        // but match explicitly so a future filter change can't turn this into a panic.
        let Some(script) = repo.dev_server_script.clone() else {
            continue;
        };
        let executor_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script,
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::DevServer,
                working_dir: Some(repo.name.clone()),
            }),
            None,
        );

        let execution_process = deployment
            .container()
            .start_execution(
                &workspace,
                &session,
                &executor_action,
                &ExecutionProcessRunReason::DevServer,
            )
            .await?;
        execution_processes.push(execution_process);
    }

    deployment
        .track_if_analytics_allowed(
            "dev_server_started",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(execution_processes)))
}

pub async fn stop_workspace_execution(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    deployment.container().try_stop(&workspace, false).await;

    deployment
        .track_if_analytics_allowed(
            "task_attempt_stopped",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// List running background helpers for the workspace.
#[axum::debug_handler]
pub async fn list_background_helpers(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ExecutionProcess>>>, ApiError> {
    let helpers = ExecutionProcess::find_running_by_workspace_and_run_reason(
        &deployment.db().pool,
        workspace.id,
        &ExecutionProcessRunReason::BackgroundHelper,
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(helpers)))
}

/// Spawn an agent-requested background helper (watcher, tunnel, log
/// follower). The helper runs in its own process group, so it survives the
/// turn-end process-group reap, and is tracked as an execution process:
/// visible in the Processes tab and stoppable via
/// `POST /api/execution-processes/{id}/stop`.
#[axum::debug_handler]
pub async fn start_background_helper(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<StartBackgroundHelperRequest>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess, StartBackgroundHelperError>>, ApiError> {
    let pool = &deployment.db().pool;

    if request.script.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            StartBackgroundHelperError::EmptyScript,
        )));
    }

    // The working dir must stay inside the workspace: relative, no `..`.
    if let Some(dir) = request.working_dir.as_deref()
        && !is_valid_helper_working_dir(dir)
    {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            StartBackgroundHelperError::InvalidWorkingDir,
        )));
    }

    let running = ExecutionProcess::find_running_by_workspace_and_run_reason(
        pool,
        workspace.id,
        &ExecutionProcessRunReason::BackgroundHelper,
    )
    .await?;
    if running.len() >= MAX_BACKGROUND_HELPERS_PER_WORKSPACE {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            StartBackgroundHelperError::TooManyHelpers,
        )));
    }

    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;

    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: None,
                    name: None,
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let executor_action = ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: request.script,
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::BackgroundHelper,
            working_dir: request.working_dir,
        }),
        None,
    );

    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::BackgroundHelper,
        )
        .await?;

    deployment
        .track_if_analytics_allowed(
            "background_helper_started",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(execution_process)))
}

#[axum::debug_handler]
pub async fn run_cleanup_script(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess, RunScriptError>>, ApiError> {
    let pool = &deployment.db().pool;

    if ExecutionProcess::has_running_non_persistent_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            RunScriptError::ProcessAlreadyRunning,
        )));
    }

    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let executor_action = match deployment.container().cleanup_actions_for_repos(&repos) {
        Some(action) => action,
        None => {
            return Ok(ResponseJson(ApiResponse::error_with_data(
                RunScriptError::NoScriptConfigured,
            )));
        }
    };

    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: None,
                    name: None,
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::CleanupScript,
        )
        .await?;

    deployment
        .track_if_analytics_allowed(
            "cleanup_script_executed",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(execution_process)))
}

pub async fn run_archive_script(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess, RunScriptError>>, ApiError> {
    let pool = &deployment.db().pool;
    if ExecutionProcess::has_running_non_persistent_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            RunScriptError::ProcessAlreadyRunning,
        )));
    }

    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let executor_action = match deployment.container().archive_actions_for_repos(&repos) {
        Some(action) => action,
        None => {
            return Ok(ResponseJson(ApiResponse::error_with_data(
                RunScriptError::NoScriptConfigured,
            )));
        }
    };
    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: None,
                    name: None,
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::ArchiveScript,
        )
        .await?;

    deployment
        .track_if_analytics_allowed(
            "archive_script_executed",
            serde_json::json!({
                "workspace_id": workspace.id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(execution_process)))
}

/// A helper's working dir must stay inside the workspace: relative, no `..`.
fn is_valid_helper_working_dir(dir: &str) -> bool {
    let path = std::path::Path::new(dir);
    !path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::is_valid_helper_working_dir;

    #[test]
    fn accepts_relative_working_dirs() {
        assert!(is_valid_helper_working_dir("frontend"));
        assert!(is_valid_helper_working_dir("packages/web-core"));
        assert!(is_valid_helper_working_dir("./frontend"));
    }

    #[test]
    fn rejects_escaping_working_dirs() {
        assert!(!is_valid_helper_working_dir("/etc"));
        assert!(!is_valid_helper_working_dir("../other-workspace"));
        assert!(!is_valid_helper_working_dir("frontend/../../escape"));
    }
}
