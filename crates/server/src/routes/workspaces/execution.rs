use axum::{
    Extension, Json, Router,
    extract::State,
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    session::{CreateSession, Session},
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::actions::{
    ExecutorAction, ExecutorActionType,
    script::{
        PollerSpec, ScriptContext, ScriptRequest, ScriptRequestLanguage, compile_poller_script,
        validate_interval,
    },
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
///
/// The cap is deliberately **shared** between plain background helpers and
/// pollers, and is not two caps of five. A poller *is* a background helper —
/// same run reason, same process group, same lifetime — that happens to retain
/// the `(command, interval)` it was created from, so it consumes the same
/// resource. Splitting this in two would quietly double the fleet an agent can
/// accumulate per workspace.
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

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum StartPollerError {
    EmptyCommand,
    /// The interval was zero, below `MIN_POLLER_INTERVAL_SECS`, or above
    /// `MAX_POLLER_INTERVAL_SECS`. Never silently defaulted: a defaulted
    /// interval is a hot loop.
    InvalidInterval,
    InvalidWorkingDir,
    TooManyHelpers,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct StartPollerRequest {
    /// Command to run on each tick.
    pub command: String,
    /// Seconds between ticks.
    pub interval_secs: u32,
    /// Optional path to run the command in, relative to the workspace root.
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// A running poller, described from the `PollerSpec` it was created with rather
/// than scraped back out of the generated loop.
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct PollerSummary {
    pub id: Uuid,
    pub status: ExecutionProcessStatus,
    pub command: String,
    pub interval_secs: u32,
    pub working_dir: Option<String>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct ListPollersResponse {
    pub pollers: Vec<PollerSummary>,
    pub count: u32,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/dev-server/start", post(start_dev_server))
        .route("/background-helpers", get(list_background_helpers))
        .route("/background-helpers/start", post(start_background_helper))
        .route("/pollers", get(list_pollers))
        .route("/pollers/start", post(start_poller))
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
                poller: None,
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

/// A reason the shared start-preamble refused to start a background process.
/// Each route maps it onto its own error enum so callers keep seeing an error
/// named for the thing they asked for.
enum HelperStartRejection {
    InvalidWorkingDir,
    TooManyHelpers,
}

impl From<HelperStartRejection> for StartBackgroundHelperError {
    fn from(rejection: HelperStartRejection) -> Self {
        match rejection {
            HelperStartRejection::InvalidWorkingDir => Self::InvalidWorkingDir,
            HelperStartRejection::TooManyHelpers => Self::TooManyHelpers,
        }
    }
}

impl From<HelperStartRejection> for StartPollerError {
    fn from(rejection: HelperStartRejection) -> Self {
        match rejection {
            HelperStartRejection::InvalidWorkingDir => Self::InvalidWorkingDir,
            HelperStartRejection::TooManyHelpers => Self::TooManyHelpers,
        }
    }
}

/// Everything a `BackgroundHelper` start has to do before it can build an
/// action: validate the working dir, check the (shared — see
/// `MAX_BACKGROUND_HELPERS_PER_WORKSPACE`) concurrency cap, make sure the
/// container exists, and resolve the session to attribute the process to.
///
/// Helpers and pollers go through this one implementation so the cap and the
/// working-dir rule cannot drift apart between the two routes.
async fn prepare_helper_start(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
    working_dir: Option<&str>,
) -> Result<Result<Session, HelperStartRejection>, ApiError> {
    let pool = &deployment.db().pool;

    // The working dir must stay inside the workspace: relative, no `..`.
    if let Some(dir) = working_dir
        && !is_valid_helper_working_dir(dir)
    {
        return Ok(Err(HelperStartRejection::InvalidWorkingDir));
    }

    let running = ExecutionProcess::find_running_by_workspace_and_run_reason(
        pool,
        workspace.id,
        &ExecutionProcessRunReason::BackgroundHelper,
    )
    .await?;
    if running.len() >= MAX_BACKGROUND_HELPERS_PER_WORKSPACE {
        return Ok(Err(HelperStartRejection::TooManyHelpers));
    }

    deployment
        .container()
        .ensure_container_exists(workspace)
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

    Ok(Ok(session))
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
    if request.script.trim().is_empty() {
        return Ok(ResponseJson(ApiResponse::error_with_data(
            StartBackgroundHelperError::EmptyScript,
        )));
    }

    let session = match prepare_helper_start(
        &deployment,
        &workspace,
        request.working_dir.as_deref(),
    )
    .await?
    {
        Ok(session) => session,
        Err(rejection) => {
            return Ok(ResponseJson(ApiResponse::error_with_data(rejection.into())));
        }
    };

    let executor_action = ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: request.script,
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::BackgroundHelper,
            working_dir: request.working_dir,
            poller: None,
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

/// List the workspace's running pollers.
///
/// A poller is a background helper that retained the spec it was created with,
/// so the filter is `run_reason == BackgroundHelper && poller.is_some()` —
/// plain helpers keep their own endpoint and are excluded here. Like
/// `GET /background-helpers`, this reports the *running* ones; finished pollers
/// stay visible in the Processes tab.
#[axum::debug_handler]
pub async fn list_pollers(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ListPollersResponse>>, ApiError> {
    let processes = ExecutionProcess::find_running_by_workspace_and_run_reason(
        &deployment.db().pool,
        workspace.id,
        &ExecutionProcessRunReason::BackgroundHelper,
    )
    .await?;

    let pollers: Vec<PollerSummary> = processes.iter().filter_map(poller_summary).collect();

    Ok(ResponseJson(ApiResponse::success(ListPollersResponse {
        count: pollers.len() as u32,
        pollers,
    })))
}

/// The database-free half of validating a poller request.
///
/// An out-of-range interval — zero included — is refused rather than clamped or
/// defaulted, because a defaulted interval is a hot loop and the agent needs to
/// be told the interval it asked for is not one it can have.
fn poller_spec_from_request(
    command: String,
    interval_secs: u32,
) -> Result<PollerSpec, StartPollerError> {
    if command.trim().is_empty() {
        return Err(StartPollerError::EmptyCommand);
    }
    if !validate_interval(interval_secs) {
        return Err(StartPollerError::InvalidInterval);
    }
    Ok(PollerSpec {
        command,
        interval_secs,
    })
}

/// Describe an execution process as a poller, or `None` if it is not one.
fn poller_summary(process: &ExecutionProcess) -> Option<PollerSummary> {
    let action = process.executor_action().ok()?;
    let ExecutorActionType::ScriptRequest(script) = action.typ() else {
        return None;
    };
    let spec = script.poller.as_ref()?;

    Some(PollerSummary {
        id: process.id,
        status: process.status.clone(),
        command: spec.command.clone(),
        interval_secs: spec.interval_secs,
        working_dir: script.working_dir.clone(),
        started_at: process.started_at,
    })
}

/// Human-readable reason for a rejected poller start.
///
/// The typed `StartPollerError` is what the frontend matches on, but non-browser
/// clients only receive `message` — an MCP tool reports a message-less error as
/// "Unknown error", which tells a calling agent nothing about whether to change
/// the interval, the working directory, or stop retrying. Each message names the
/// failure and the corrective action (Constitution XXI).
fn start_poller_error_message(error: &StartPollerError) -> &'static str {
    match error {
        StartPollerError::EmptyCommand => {
            "Poller command is empty: supply the command to run on each tick."
        }
        StartPollerError::InvalidInterval => {
            "Poller interval is out of range: interval_secs must be between 5 and 86400 seconds. It is never defaulted, because a defaulted interval is a hot loop."
        }
        StartPollerError::InvalidWorkingDir => {
            "Poller working_dir is invalid: it must be relative to the workspace root and must not contain '..'."
        }
        StartPollerError::TooManyHelpers => {
            "Too many background processes in this workspace: pollers and background helpers share one limit of 5. Stop an existing poller or helper first."
        }
    }
}

/// Start a poller: a background helper whose script is a generated loop running
/// the agent's command on an interval. It shares the helper lifetime exactly —
/// own process group, survives the turn-end reap and server restarts, stopped
/// by workspace archive or `POST /api/execution-processes/{id}/stop`.
#[axum::debug_handler]
pub async fn start_poller(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<StartPollerRequest>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess, StartPollerError>>, ApiError> {
    let spec = match poller_spec_from_request(request.command, request.interval_secs) {
        Ok(spec) => spec,
        Err(error) => {
            let message = start_poller_error_message(&error);
            return Ok(ResponseJson(ApiResponse::error_with_data_and_message(
                error, message,
            )));
        }
    };

    let session = match prepare_helper_start(
        &deployment,
        &workspace,
        request.working_dir.as_deref(),
    )
    .await?
    {
        Ok(session) => session,
        Err(rejection) => {
            let error: StartPollerError = rejection.into();
            let message = start_poller_error_message(&error);
            return Ok(ResponseJson(ApiResponse::error_with_data_and_message(
                error, message,
            )));
        }
    };

    let executor_action = ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: compile_poller_script(&spec),
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::BackgroundHelper,
            working_dir: request.working_dir,
            poller: Some(spec),
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
            "poller_started",
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
    use db::models::{
        execution_process::CreateExecutionProcess, session::CreateSession,
        workspace::CreateWorkspace,
    };
    use executors::actions::script::{MAX_POLLER_INTERVAL_SECS, MIN_POLLER_INTERVAL_SECS};

    use super::*;

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

    #[test]
    fn an_out_of_range_interval_surfaces_as_invalid_interval() {
        for interval in [
            0,
            MIN_POLLER_INTERVAL_SECS - 1,
            MAX_POLLER_INTERVAL_SECS + 1,
        ] {
            assert!(
                matches!(
                    poller_spec_from_request("echo hi".to_string(), interval),
                    Err(StartPollerError::InvalidInterval)
                ),
                "interval {interval} should be rejected, not defaulted"
            );
        }

        let spec = poller_spec_from_request("echo hi".to_string(), MIN_POLLER_INTERVAL_SECS)
            .expect("the minimum interval is valid");
        assert_eq!(spec.interval_secs, MIN_POLLER_INTERVAL_SECS);
    }

    #[test]
    fn an_empty_command_surfaces_as_empty_command() {
        assert!(matches!(
            poller_spec_from_request("   ".to_string(), 60),
            Err(StartPollerError::EmptyCommand)
        ));
    }

    /// Asserting the typed variant is not enough: an MCP client only receives
    /// `message`, so a rejection that carries only `error_data` reaches a
    /// calling agent as "Unknown error" and it cannot tell what to change. This
    /// was found by driving the shipped `spawn_poller` tool, not by the variant
    /// assertions above — which passed the whole time.
    #[test]
    fn every_rejection_carries_a_message_that_names_the_problem() {
        let cases = [
            (StartPollerError::EmptyCommand, "empty"),
            (StartPollerError::InvalidInterval, "interval_secs"),
            (StartPollerError::InvalidWorkingDir, "working_dir"),
            (StartPollerError::TooManyHelpers, "limit of 5"),
        ];

        for (error, expected_fragment) in cases {
            let message = start_poller_error_message(&error);
            assert!(
                message.contains(expected_fragment),
                "{error:?} message should name the problem ({expected_fragment:?}), got {message:?}"
            );

            // The message is what a non-browser client actually surfaces, so it
            // has to survive onto the response envelope, not just exist.
            let response: ApiResponse<ExecutionProcess, StartPollerError> =
                ApiResponse::error_with_data_and_message(error, message);
            assert_eq!(response.message(), Some(message));
        }
    }

    #[test]
    fn an_escaping_working_dir_is_refused_for_pollers_as_well() {
        // Pollers reuse the helper rule rather than re-deriving one, so the
        // contract under test is that the shared rejection keeps its name when
        // it reaches a poller caller.
        assert!(!is_valid_helper_working_dir("../other-workspace"));
        assert!(matches!(
            StartPollerError::from(HelperStartRejection::InvalidWorkingDir),
            StartPollerError::InvalidWorkingDir
        ));
        assert!(matches!(
            StartBackgroundHelperError::from(HelperStartRejection::InvalidWorkingDir),
            StartBackgroundHelperError::InvalidWorkingDir
        ));
    }

    // -- test helpers -----------------------------------------------------

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        pool
    }

    async fn workspace_session(pool: &sqlx::SqlitePool) -> Session {
        let workspace = Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "vk/poller-test".to_string(),
                name: Some("poller-test".to_string()),
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();

        Session::create(
            pool,
            &CreateSession {
                executor: None,
                name: None,
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap()
    }

    async fn start_helper(pool: &sqlx::SqlitePool, session: &Session, poller: Option<PollerSpec>) {
        let script = match &poller {
            Some(spec) => compile_poller_script(spec),
            None => "npm run watch".to_string(),
        };
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script,
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::BackgroundHelper,
                        working_dir: None,
                        poller,
                    }),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::BackgroundHelper,
            },
            Uuid::new_v4(),
            &[],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn the_concurrency_cap_counts_helpers_and_pollers_together() {
        let pool = test_pool().await;
        let session = workspace_session(&pool).await;

        for _ in 0..3 {
            start_helper(&pool, &session, None).await;
        }
        for _ in 0..2 {
            start_helper(
                &pool,
                &session,
                Some(PollerSpec {
                    command: "git fetch --dry-run".to_string(),
                    interval_secs: 60,
                }),
            )
            .await;
        }

        // This is the count the shared start-preamble checks. Helpers and
        // pollers share one budget on purpose (see
        // MAX_BACKGROUND_HELPERS_PER_WORKSPACE); three plus two must reach the
        // cap of five, not sit at three of five.
        let running = ExecutionProcess::find_running_by_workspace_and_run_reason(
            &pool,
            session.workspace_id,
            &ExecutionProcessRunReason::BackgroundHelper,
        )
        .await
        .unwrap();

        assert_eq!(running.len(), 5);
        assert!(running.len() >= MAX_BACKGROUND_HELPERS_PER_WORKSPACE);
    }

    #[tokio::test]
    async fn listing_pollers_excludes_plain_background_helpers() {
        let pool = test_pool().await;
        let session = workspace_session(&pool).await;

        start_helper(&pool, &session, None).await;
        start_helper(
            &pool,
            &session,
            Some(PollerSpec {
                command: "git fetch --dry-run origin main".to_string(),
                interval_secs: 90,
            }),
        )
        .await;

        let running = ExecutionProcess::find_running_by_workspace_and_run_reason(
            &pool,
            session.workspace_id,
            &ExecutionProcessRunReason::BackgroundHelper,
        )
        .await
        .unwrap();
        let pollers: Vec<PollerSummary> = running.iter().filter_map(poller_summary).collect();

        assert_eq!(pollers.len(), 1);
        // Described from the retained spec, not scraped out of the generated
        // loop.
        assert_eq!(pollers[0].command, "git fetch --dry-run origin main");
        assert_eq!(pollers[0].interval_secs, 90);
        assert_eq!(pollers[0].status, ExecutionProcessStatus::Running);
    }
}
