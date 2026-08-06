use axum::{Extension, Json, extract::State, response::Json as ResponseJson};
use chrono::Utc;
use db::models::{
    coding_agent_turn::CodingAgentTurn,
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    execution_worker_job::{ExecutionWorkerDispatchState, ExecutionWorkerJob},
    session::Session,
    worker_node::WorkerNode,
    workspace::{Workspace, WorkspacePlacement, WorkspacePlacementState},
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
    },
    executors::BaseCodingAgent, profile::ExecutorConfig,
};
use cluster_protocol::{
    CodexRolloutManifestRequest, CodexRolloutReadRequest, CodexRolloutStageRequest,
    CodexRolloutVerifyRequest, ExecutionQuiescenceRequest, PROTOCOL_VERSION, RequestAuthority,
};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub const WORKSPACE_AFFINITY_MIGRATION_PROMPT: &str = "Vibe Kanban moved this workspace to another execution server. Review the current working tree, git state, and prior conversation, then continue the unfinished task without repeating work that is already complete. Preserve existing user changes and report any state you cannot reconcile.";
const ABANDONED_OPERATION_MINUTES: i64 = 10;

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateWorkspaceAffinityRequest {
    /// Explicitly move the workspace back to coordinator-local execution.
    #[serde(default)]
    pub run_on_coordinator: bool,
    pub requested_worker_node_id: Option<Uuid>,
    #[serde(default)]
    pub restart_running: bool,
    pub operation_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AffinityIntent {
    Automatic,
    Coordinator,
    Worker(Uuid),
}

impl AffinityIntent {
    #[allow(clippy::result_large_err)]
    fn resolve(request: &UpdateWorkspaceAffinityRequest) -> Result<Self, ApiError> {
        match (request.run_on_coordinator, request.requested_worker_node_id) {
            (true, Some(_)) => Err(ApiError::BadRequest(
                "run_on_coordinator cannot be combined with requested_worker_node_id".into(),
            )),
            (true, None) => Ok(Self::Coordinator),
            (false, Some(worker_node_id)) => Ok(Self::Worker(worker_node_id)),
            (false, None) => Ok(Self::Automatic),
        }
    }

    fn requested_worker_node_id(self) -> Option<Uuid> {
        match self {
            Self::Worker(worker_node_id) => Some(worker_node_id),
            Self::Automatic | Self::Coordinator => None,
        }
    }

    fn matches(self, placement: &WorkspacePlacement) -> bool {
        match self {
            Self::Coordinator => placement.placement_state == WorkspacePlacementState::Local,
            Self::Automatic => {
                placement.placement_state != WorkspacePlacementState::Local
                    && placement.worker_node_id.is_some()
                    && placement.requested_worker_node_id.is_none()
            }
            Self::Worker(worker_node_id) => {
                placement.placement_state != WorkspacePlacementState::Local
                    && placement.worker_node_id == Some(worker_node_id)
                    && placement.requested_worker_node_id == Some(worker_node_id)
            }
        }
    }
}

fn operation_matches_request(
    stored_worker_id: Option<Uuid>,
    stored_coordinator: bool,
    stored_restart: bool,
    request: &UpdateWorkspaceAffinityRequest,
) -> bool {
    stored_worker_id == request.requested_worker_node_id
        && stored_coordinator == request.run_on_coordinator
        && stored_restart == request.restart_running
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[ts(use_ts_enum)]
pub enum WorkspaceAffinityUpdateOutcome {
    Updated,
    Restarted,
    RestartFailed,
    SessionTransferFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceAffinityUpdateResponse {
    pub placement: WorkspacePlacement,
    pub outcome: WorkspaceAffinityUpdateOutcome,
    pub stopped_execution_id: Option<Uuid>,
    pub started_execution: Option<ExecutionProcess>,
    pub message: Option<String>,
}

#[allow(clippy::result_large_err)]
fn executor_config(process: &ExecutionProcess) -> Result<ExecutorConfig, ApiError> {
    let action = process.executor_action().map_err(|error| {
        ApiError::Conflict(format!(
            "Could not read executor configuration for {}: {error}",
            process.id
        ))
    })?;
    match action.typ() {
        ExecutorActionType::CodingAgentInitialRequest(request) => {
            Ok(request.executor_config.clone())
        }
        ExecutorActionType::CodingAgentFollowUpRequest(request) => {
            Ok(request.executor_config.clone())
        }
        _ => Err(ApiError::Conflict(format!(
            "Running execution {} is not a resumable coding-agent task",
            process.id
        ))),
    }
}

fn transfer_authority(
    coordinator_id: Uuid,
    worker_node_id: Uuid,
    operation_id: Uuid,
) -> RequestAuthority {
    RequestAuthority {
        protocol_version: PROTOCOL_VERSION,
        coordinator_id,
        worker_node_id,
        correlation_id: operation_id,
        issued_at: Utc::now(),
        nonce: Uuid::new_v4().to_string(),
    }
}

async fn transfer_codex_rollouts(
    deployment: &DeploymentImpl,
    pool: &sqlx::SqlitePool,
    operation_id: Uuid,
    workspace_id: Uuid,
    source_execution_id: Uuid,
    source_worker_node_id: Uuid,
    target_worker_node_id: Uuid,
    leaf_thread_id: Uuid,
) -> Result<(), ApiError> {
    let coordinator_id = deployment.cluster_config().coordinator_id.ok_or_else(|| {
        ApiError::Conflict("Codex session transfer has no coordinator identity".into())
    })?;
    let client = deployment.worker_client().ok_or_else(|| {
        ApiError::Conflict("Codex session transfer client is unavailable".into())
    })?;
    let quiescence = ExecutionQuiescenceRequest {
        authority: transfer_authority(coordinator_id, source_worker_node_id, operation_id),
        operation_id,
        execution_id: source_execution_id,
        workspace_id,
    };
    client
        .set_execution_quiesced(source_worker_node_id, &quiescence, true)
        .await
        .map_err(|error| ApiError::Conflict(format!(
            "Codex session transfer could not quiesce source execution {source_execution_id}: {error}"
        )))?;

    let transfer = async {
        let manifest = client
            .codex_rollout_manifest(
                source_worker_node_id,
                &CodexRolloutManifestRequest {
                    authority: transfer_authority(
                        coordinator_id,
                        source_worker_node_id,
                        operation_id,
                    ),
                    operation_id,
                    workspace_id,
                    source_execution_id,
                    source_worker_node_id,
                    target_worker_node_id,
                    leaf_thread_id,
                },
            )
            .await
            .map_err(|error| ApiError::Conflict(format!(
                "Codex session transfer could not resolve rollout lineage for thread {leaf_thread_id}: {error}"
            )))?;
        let manifest_json = serde_json::to_string(&manifest).map_err(|_| {
            ApiError::Conflict("Codex session transfer manifest could not be persisted".into())
        })?;
        sqlx::query(
            r#"UPDATE workspace_affinity_operations
               SET session_transfer_manifest_json = ?,
                   session_transfer_manifest_sha256 = ?,
                   updated_at = datetime('now', 'subsec')
               WHERE operation_id = ? AND status = 'claimed'
                 AND (session_transfer_manifest_sha256 IS NULL
                      OR session_transfer_manifest_sha256 = ?)"#,
        )
        .bind(&manifest_json)
        .bind(&manifest.manifest_sha256)
        .bind(operation_id)
        .bind(&manifest.manifest_sha256)
        .execute(pool)
        .await?;

        for entry in &manifest.entries {
            let artifact = client
                .codex_rollout_artifact(
                    source_worker_node_id,
                    &CodexRolloutReadRequest {
                        authority: transfer_authority(
                            coordinator_id,
                            source_worker_node_id,
                            operation_id,
                        ),
                        manifest: manifest.clone(),
                        thread_id: entry.thread_id,
                    },
                )
                .await
                .map_err(|error| ApiError::Conflict(format!(
                    "Codex session transfer could not read rollout for thread {}: {error}",
                    entry.thread_id
                )))?;
            client
                .stage_codex_rollout(
                    target_worker_node_id,
                    &CodexRolloutStageRequest {
                        authority: transfer_authority(
                            coordinator_id,
                            target_worker_node_id,
                            operation_id,
                        ),
                        manifest: manifest.clone(),
                        artifact,
                    },
                )
                .await
                .map_err(|error| ApiError::Conflict(format!(
                    "Codex session transfer could not stage rollout for thread {}: {error}",
                    entry.thread_id
                )))?;
        }
        let verification = client
            .verify_codex_rollouts(
                target_worker_node_id,
                &CodexRolloutVerifyRequest {
                    authority: transfer_authority(
                        coordinator_id,
                        target_worker_node_id,
                        operation_id,
                    ),
                    manifest: manifest.clone(),
                },
            )
            .await
            .map_err(|error| ApiError::Conflict(format!(
                "Codex session transfer target verification failed for thread {leaf_thread_id}: {error}"
            )))?;
        if verification.manifest_sha256 != manifest.manifest_sha256
            || verification.verified_thread_ids.len() != manifest.entries.len()
        {
            return Err(ApiError::Conflict(format!(
                "Codex session transfer target returned incomplete verification for thread {leaf_thread_id}"
            )));
        }
        let updated = sqlx::query(
            r#"UPDATE workspace_affinity_operations
               SET session_transfer_verified_at = datetime('now', 'subsec'),
                   updated_at = datetime('now', 'subsec')
               WHERE operation_id = ? AND status = 'claimed'
                 AND session_transfer_manifest_sha256 = ?"#,
        )
        .bind(operation_id)
        .bind(&manifest.manifest_sha256)
        .execute(pool)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::Conflict(
                "Codex session transfer verification could not be recorded".into(),
            ));
        }
        Ok(())
    }
    .await;

    if transfer.is_err() {
        let resume = ExecutionQuiescenceRequest {
            authority: transfer_authority(coordinator_id, source_worker_node_id, operation_id),
            operation_id,
            execution_id: source_execution_id,
            workspace_id,
        };
        if let Err(resume_error) = client
            .set_execution_quiesced(source_worker_node_id, &resume, false)
            .await
        {
            return Err(ApiError::Conflict(format!(
                "Codex session transfer failed and source execution {source_execution_id} could not be resumed: {resume_error}"
            )));
        }
    }
    transfer
}

async fn replay_operation(
    pool: &sqlx::SqlitePool,
    operation_id: Uuid,
    workspace_id: Uuid,
    request: &UpdateWorkspaceAffinityRequest,
) -> Result<Option<WorkspaceAffinityUpdateResponse>, ApiError> {
    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<Uuid>,
            Option<Uuid>,
            bool,
            bool,
            String,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"SELECT workspace_id, source_execution_id, requested_worker_node_id,
                  run_on_coordinator, restart_running,
                  status, result_json, error_message
           FROM workspace_affinity_operations WHERE operation_id = ?"#,
    )
    .bind(operation_id)
    .fetch_optional(pool)
    .await?;

    let Some((
        stored_workspace_id,
        source_execution_id,
        stored_worker_id,
        stored_coordinator,
        stored_restart,
        status,
        result,
        error,
    )) = row
    else {
        return Ok(None);
    };
    if stored_workspace_id != workspace_id
        || !operation_matches_request(
            stored_worker_id,
            stored_coordinator,
            stored_restart,
            request,
        )
    {
        return Err(ApiError::BadRequest(
            "Affinity operation id was already used for a different request".into(),
        ));
    }
    match status.as_str() {
        "completed" => result
            .as_deref()
            .ok_or_else(|| ApiError::Conflict("Completed affinity operation has no result".into()))
            .and_then(|json| {
                serde_json::from_str(json).map_err(|error| {
                    ApiError::Conflict(format!("Stored affinity result is invalid: {error}"))
                })
            })
            .map(Some),
        "failed" => {
            Err(ApiError::Conflict(error.unwrap_or_else(|| {
                "The previous affinity operation failed".into()
            })))
        }
        _ => {
            // The operation id is also the continuation execution id. This
            // closes the crash window between starting the agent and storing
            // the HTTP result: a retry recovers the already-created process.
            if let Some(started) = ExecutionProcess::find_by_id(pool, operation_id).await? {
                let placement = WorkspacePlacement::find(pool, workspace_id)
                    .await?
                    .ok_or_else(|| {
                        ApiError::Conflict("Workspace placement was not found".into())
                    })?;
                let dispatch = ExecutionWorkerJob::find_by_execution_id(pool, operation_id).await?;
                let process_is_viable = !matches!(
                    started.status,
                    ExecutionProcessStatus::Failed
                        | ExecutionProcessStatus::Killed
                        | ExecutionProcessStatus::Interrupted
                        | ExecutionProcessStatus::Indeterminate
                );
                let restart_confirmed = if request.run_on_coordinator {
                    process_is_viable
                } else {
                    dispatch.as_ref().is_some_and(|job| {
                        matches!(
                            job.dispatch_state,
                            ExecutionWorkerDispatchState::Accepted
                                | ExecutionWorkerDispatchState::Starting
                                | ExecutionWorkerDispatchState::Running
                                | ExecutionWorkerDispatchState::Completed
                        )
                    }) && process_is_viable
                };
                let restart_conclusively_failed = matches!(
                    started.status,
                    ExecutionProcessStatus::Failed
                        | ExecutionProcessStatus::Killed
                        | ExecutionProcessStatus::Interrupted
                        | ExecutionProcessStatus::Indeterminate
                ) || dispatch
                    .as_ref()
                    .is_some_and(|job| job.dispatch_state.is_terminal());
                if !restart_confirmed && !restart_conclusively_failed {
                    // Creation/dispatch may still be in flight. Keep the claim
                    // active; a stale retry will resume the durable operation.
                    return Ok(None);
                }
                let recovered = WorkspaceAffinityUpdateResponse {
                    placement,
                    outcome: if restart_confirmed {
                        WorkspaceAffinityUpdateOutcome::Restarted
                    } else {
                        WorkspaceAffinityUpdateOutcome::RestartFailed
                    },
                    stopped_execution_id: source_execution_id,
                    started_execution: restart_confirmed.then_some(started),
                    message: Some(if restart_confirmed {
                        "Recovered the completed migration after a response interruption".into()
                    } else {
                        "Affinity changed, but the interrupted continuation was not confirmed dispatched"
                            .into()
                    }),
                };
                finish_operation(pool, operation_id, &recovered).await?;
                Ok(Some(recovered))
            } else {
                Ok(None)
            }
        }
    }
}

async fn claim_operation(
    pool: &sqlx::SqlitePool,
    operation_id: Uuid,
    workspace_id: Uuid,
    request: &UpdateWorkspaceAffinityRequest,
    source_execution_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let inserted = sqlx::query(
        r#"INSERT OR IGNORE INTO workspace_affinity_operations
           (operation_id, workspace_id, source_execution_id, requested_worker_node_id,
            run_on_coordinator, restart_running)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(operation_id)
    .bind(workspace_id)
    .bind(source_execution_id)
    .bind(request.requested_worker_node_id)
    .bind(request.run_on_coordinator)
    .bind(request.restart_running)
    .execute(pool)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "A server affinity migration is already in progress for this workspace".into(),
        ));
    }
    Ok(())
}

async fn finish_operation(
    pool: &sqlx::SqlitePool,
    operation_id: Uuid,
    result: &WorkspaceAffinityUpdateResponse,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"UPDATE workspace_affinity_operations
           SET status = 'completed', result_json = ?, updated_at = datetime('now', 'subsec')
           WHERE operation_id = ?"#,
    )
    .bind(serde_json::to_string(result).map_err(|error| {
        ApiError::Conflict(format!(
            "Could not persist affinity operation result: {error}"
        ))
    })?)
    .bind(operation_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn touch_operation(pool: &sqlx::SqlitePool, operation_id: Uuid) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE workspace_affinity_operations SET updated_at = datetime('now', 'subsec') WHERE operation_id = ? AND status = 'claimed'",
    )
    .bind(operation_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_source_stop_started(
    pool: &sqlx::SqlitePool,
    operation_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"UPDATE workspace_affinity_operations
           SET source_stop_started = 1, updated_at = datetime('now', 'subsec')
           WHERE operation_id = ? AND status = 'claimed'"#,
    )
    .bind(operation_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fail_operation(pool: &sqlx::SqlitePool, operation_id: Uuid, error: &ApiError) {
    let _ = sqlx::query(
        r#"UPDATE workspace_affinity_operations
           SET status = 'failed', error_message = ?, updated_at = datetime('now', 'subsec')
           WHERE operation_id = ?"#,
    )
    .bind(error.to_string())
    .bind(operation_id)
    .execute(pool)
    .await;
}

async fn expire_abandoned_operations(
    pool: &sqlx::SqlitePool,
    workspace_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query(
        r#"UPDATE workspace_affinity_operations
           SET status = 'failed',
               error_message = 'Affinity migration expired after its coordinator was interrupted; retry with a new operation id',
               updated_at = datetime('now', 'subsec')
           WHERE workspace_id = ? AND status = 'claimed'
             AND (restart_running = 0 OR source_execution_id IS NULL)
             AND updated_at <= datetime('now', ? || ' minutes')"#,
    )
    .bind(workspace_id)
    .bind(-ABANDONED_OPERATION_MINUTES)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_workspace_affinity(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(mut request): Json<UpdateWorkspaceAffinityRequest>,
) -> Result<ResponseJson<ApiResponse<WorkspaceAffinityUpdateResponse>>, ApiError> {
    if !deployment.cluster_config().enabled {
        return Err(ApiError::BadRequest(
            "Server affinity is read-only when clustered execution is disabled".into(),
        ));
    }
    let mut intent = AffinityIntent::resolve(&request)?;
    let pool = &deployment.db().pool;
    let placement = WorkspacePlacement::find(pool, workspace.id)
        .await?
        .ok_or_else(|| ApiError::Conflict("Workspace placement was not found".into()))?;

    // A process crash cannot be allowed to retain the partial unique index
    // forever. Active coordinators finish well inside this lease and stale
    // claims become a durable failure that permits a fresh operation id.
    expire_abandoned_operations(pool, workspace.id).await?;

    let mut resumed_source_execution_id = None;
    let mut resuming_operation_id = None;
    if let Some((
        active_operation_id,
        stored_worker_id,
        stored_coordinator,
        stored_restart,
        source_execution_id,
        stale,
    )) = sqlx::query_as::<_, (Uuid, Option<Uuid>, bool, bool, Option<Uuid>, bool)>(
        r#"SELECT operation_id, requested_worker_node_id, run_on_coordinator,
                  restart_running,
                  source_execution_id,
                  updated_at <= datetime('now', '-10 minutes')
           FROM workspace_affinity_operations
           WHERE workspace_id = ? AND status = 'claimed'"#,
    )
    .bind(workspace.id)
    .fetch_optional(pool)
    .await?
    {
        let stored_request = UpdateWorkspaceAffinityRequest {
            run_on_coordinator: stored_coordinator,
            requested_worker_node_id: stored_worker_id,
            restart_running: stored_restart,
            operation_id: Some(active_operation_id),
        };
        if let Some(replayed) =
            replay_operation(pool, active_operation_id, workspace.id, &stored_request).await?
        {
            return Ok(ResponseJson(ApiResponse::success(replayed)));
        }
        if stale && stored_restart && source_execution_id.is_some() {
            request = stored_request;
            intent = AffinityIntent::resolve(&request)?;
            resumed_source_execution_id = source_execution_id;
            resuming_operation_id = Some(active_operation_id);
        } else {
            return Err(ApiError::Conflict(
                "A server affinity migration is already in progress for this workspace".into(),
            ));
        }
    }

    // Durable replay precedes every state-based shortcut. A successful prior
    // request may already have made the placement look like a no-op while its
    // response (including the continuation identity) was lost in transit.
    if let Some(operation_id) = request.operation_id
        && let Some(replayed) = replay_operation(pool, operation_id, workspace.id, &request).await?
    {
        return Ok(ResponseJson(ApiResponse::success(replayed)));
    }

    if resuming_operation_id.is_none() && intent.matches(&placement) {
        return Ok(ResponseJson(ApiResponse::success(
            WorkspaceAffinityUpdateResponse {
                placement,
                outcome: WorkspaceAffinityUpdateOutcome::Updated,
                stopped_execution_id: None,
                started_execution: None,
                message: None,
            },
        )));
    }

    let all_running = ExecutionProcess::find_all_running_by_workspace(pool, workspace.id).await?;
    let running: Vec<_> = all_running
        .iter()
        .filter(|process| process.run_reason == ExecutionProcessRunReason::CodingAgent)
        .cloned()
        .collect();
    if all_running.len() != running.len() {
        return Err(ApiError::Conflict(
            "Stop running lifecycle scripts, dev servers, and background helpers before changing server affinity".into(),
        ));
    }
    if running.len() > 1 {
        return Err(ApiError::Conflict(
            "More than one coding-agent execution is running; affinity was not changed".into(),
        ));
    }
    let operation_id = request.operation_id.unwrap_or_else(Uuid::new_v4);
    if request.restart_running && request.operation_id.is_none() {
        return Err(ApiError::BadRequest(
            "operation_id is required when restarting a running task".into(),
        ));
    }
    // Replay before revalidating the worker inventory. Once an operation is
    // complete, its durable result remains authoritative even if that worker's
    // lease changes before an HTTP retry reaches us.
    if let Some(replayed) = replay_operation(pool, operation_id, workspace.id, &request).await? {
        return Ok(ResponseJson(ApiResponse::success(replayed)));
    }

    let reference_process = if let Some(process) = running.first() {
        Some(process.clone())
    } else if intent == AffinityIntent::Coordinator {
        // An idle coordinator migration does not schedule a worker or restart
        // an execution, so it needs no historical executor configuration.
        None
    } else {
        Some(
            ExecutionProcess::find_latest_by_workspace_and_run_reason(
                pool,
                workspace.id,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?
            .ok_or_else(|| {
                ApiError::Conflict(
                    "No coding-agent execution exists to determine worker capabilities".into(),
                )
            })?,
        )
    };
    let config = reference_process
        .as_ref()
        .map(executor_config)
        .transpose()?;
    let target_worker_node_id = if intent == AffinityIntent::Coordinator {
        None
    } else {
        let workers = WorkerNode::fetch_all(pool).await?;
        let config = config.as_ref().ok_or_else(|| {
            ApiError::Conflict(
                "No coding-agent execution exists to determine worker capabilities".into(),
            )
        })?;
        let selected = deployment
            .worker_scheduler()
            .select(
                &workers,
                &config.profile_id().to_string(),
                intent.requested_worker_node_id(),
                Utc::now(),
            )
            .map_err(|error| ApiError::BadRequest(error.to_string()))?;
        Some(selected.id)
    };
    let changes_effective_target = match target_worker_node_id {
        Some(worker_node_id) => placement.worker_node_id != Some(worker_node_id),
        None => placement.placement_state != WorkspacePlacementState::Local,
    };
    if changes_effective_target && !running.is_empty() && !request.restart_running {
        return Err(ApiError::Conflict(
            "The current task is running; confirm stop, migrate, and restart".into(),
        ));
    }

    if resuming_operation_id.is_some()
        && let Some(orphan) = ExecutionProcess::find_by_id(pool, operation_id).await?
    {
        let dispatch = ExecutionWorkerJob::find_by_execution_id(pool, operation_id).await?;
        let dispatch_needs_retry = dispatch
            .as_ref()
            .is_none_or(|job| job.dispatch_state == ExecutionWorkerDispatchState::Pending);
        if dispatch_needs_retry {
            if changes_effective_target {
                return Err(ApiError::Conflict(
                    "Interrupted continuation exists before the target placement was committed"
                        .into(),
                ));
            }
            let action = orphan.executor_action().map_err(|error| {
                ApiError::Conflict(format!(
                    "Could not recover interrupted continuation action: {error}"
                ))
            })?;
            let dispatch_result = deployment
                .container()
                .dispatch_execution(&workspace, &orphan, action)
                .await;
            let (outcome, started_execution, message) = match dispatch_result {
                Ok(()) => (
                    WorkspaceAffinityUpdateOutcome::Restarted,
                    Some(orphan),
                    Some("Recovered and dispatched the interrupted continuation".into()),
                ),
                Err(error) => {
                    ExecutionProcess::update_completion(
                        pool,
                        operation_id,
                        ExecutionProcessStatus::Failed,
                        None,
                    )
                    .await?;
                    (
                        WorkspaceAffinityUpdateOutcome::RestartFailed,
                        None,
                        Some(format!(
                            "Affinity changed, but the interrupted continuation could not be dispatched: {error}"
                        )),
                    )
                }
            };
            let recovered = WorkspaceAffinityUpdateResponse {
                placement,
                outcome,
                stopped_execution_id: resumed_source_execution_id,
                started_execution,
                message,
            };
            finish_operation(pool, operation_id, &recovered).await?;
            return Ok(ResponseJson(ApiResponse::success(recovered)));
        }
    }

    if resuming_operation_id.is_none() {
        claim_operation(
            pool,
            operation_id,
            workspace.id,
            &request,
            running.first().map(|process| process.id),
        )
        .await?;
    }

    let result = async {
        // The claim serializes coordinators, but validation above happened
        // before it was acquired. Re-read every destructive precondition so a
        // waiter cannot stop a continuation created by the operation ahead of
        // it and only then discover a stale placement.
        let claimed_placement = WorkspacePlacement::find(pool, workspace.id)
            .await?
            .ok_or_else(|| ApiError::Conflict("Workspace placement was not found".into()))?;
        let claimed_active =
            ExecutionProcess::find_all_running_by_workspace(pool, workspace.id).await?;
        let mut expected_ids: Vec<_> = running.iter().map(|process| process.id).collect();
        let mut claimed_ids: Vec<_> = claimed_active.iter().map(|process| process.id).collect();
        expected_ids.sort_unstable();
        claimed_ids.sort_unstable();
        if claimed_placement.worker_node_id != placement.worker_node_id
            || claimed_placement.requested_worker_node_id
                != placement.requested_worker_node_id
            || claimed_placement.placement_state != placement.placement_state
            || claimed_ids != expected_ids
        {
            return Err(ApiError::Conflict(
                "Workspace placement or execution state changed while affinity was queued; refresh and retry"
                    .into(),
            ));
        }

        if changes_effective_target
            && config
                .as_ref()
                .is_some_and(|config| config.executor == BaseCodingAgent::Codex)
            && let Some(selected_worker_id) = target_worker_node_id
            && let Some(process) = running.first()
        {
            let source_worker_node_id = claimed_placement.worker_node_id.ok_or_else(|| {
                ApiError::Conflict(
                    "Codex session transfer requires a source worker placement".into(),
                )
            })?;
            let session_info = CodingAgentTurn::find_latest_session_info(
                pool,
                process.session_id,
            )
            .await?
            .ok_or_else(|| {
                ApiError::Conflict(
                    "Codex session transfer source has no persisted thread identity".into(),
                )
            })?;
            let leaf_thread_id = Uuid::parse_str(&session_info.session_id).map_err(|_| {
                ApiError::Conflict(format!(
                    "Codex session transfer source thread identity is invalid for execution {}",
                    process.id
                ))
            })?;
            if let Err(error) = transfer_codex_rollouts(
                &deployment,
                pool,
                operation_id,
                workspace.id,
                process.id,
                source_worker_node_id,
                selected_worker_id,
                leaf_thread_id,
            )
            .await
            {
                return Ok(WorkspaceAffinityUpdateResponse {
                    placement: claimed_placement,
                    outcome: WorkspaceAffinityUpdateOutcome::SessionTransferFailed,
                    stopped_execution_id: None,
                    started_execution: None,
                    message: Some(error.to_string()),
                });
            }
        }

        let stopped_execution_id = if changes_effective_target
            && let Some(process) = running.first()
        {
            mark_source_stop_started(pool, operation_id).await?;
            if let Err(error) = deployment
                .container()
                .stop_execution(process, ExecutionProcessStatus::Killed)
                .await
            {
                // Local teardown marks the row before killing the child. If
                // the kill then fails, restore an active/unknown state so a
                // retry cannot migrate while the old process may still live.
                ExecutionProcess::update_completion(
                    pool,
                    process.id,
                    ExecutionProcessStatus::Indeterminate,
                    None,
                )
                .await?;
                return Err(ApiError::from(error));
            }
            let stopped = ExecutionProcess::find_by_id(pool, process.id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Stopped execution disappeared".into()))?;
            if matches!(
                stopped.status,
                ExecutionProcessStatus::Running | ExecutionProcessStatus::Indeterminate
            ) {
                return Err(ApiError::Conflict(
                    "The running task could not be confirmed stopped; affinity is unchanged".into(),
                ));
            }
            touch_operation(pool, operation_id).await?;
            Some(process.id)
        } else if let Some(source_execution_id) = resumed_source_execution_id {
            let source = ExecutionProcess::find_by_id(pool, source_execution_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Migration source execution disappeared".into()))?;
            if matches!(
                source.status,
                ExecutionProcessStatus::Running | ExecutionProcessStatus::Indeterminate
            ) {
                return Err(ApiError::Conflict(
                    "The interrupted affinity migration still has an active source execution"
                        .into(),
                ));
            }
            Some(source_execution_id)
        } else {
            None
        };

        let reassigned = if let Some(worker_node_id) = target_worker_node_id {
            WorkspacePlacement::reassign(
                pool,
                workspace.id,
                placement.worker_node_id,
                worker_node_id,
                intent.requested_worker_node_id(),
                if intent.requested_worker_node_id().is_some() {
                    "manual worker affinity update"
                } else {
                    "automatic worker affinity update"
                },
            )
            .await?
        } else {
            WorkspacePlacement::reassign_to_coordinator(
                pool,
                workspace.id,
                placement.worker_node_id,
                "coordinator affinity update",
            )
            .await?
        };
        if !reassigned {
            return Err(ApiError::Conflict(
                "Workspace placement changed while affinity was being updated; refresh and retry"
                    .into(),
            ));
        }
        let updated_placement = WorkspacePlacement::find(pool, workspace.id)
            .await?
            .ok_or_else(|| ApiError::Conflict("Updated placement was not found".into()))?;
        touch_operation(pool, operation_id).await?;

        if stopped_execution_id.is_none() {
            return Ok(WorkspaceAffinityUpdateResponse {
                placement: updated_placement,
                outcome: WorkspaceAffinityUpdateOutcome::Updated,
                stopped_execution_id,
                started_execution: None,
                message: None,
            });
        }

        // Placement is now committed. Every subsequent failure is partial
        // success and must preserve that fact in the durable response.
        let restart = async {
            let reference_process = reference_process.as_ref().ok_or_else(|| {
                ApiError::Conflict("Migration restart has no source execution".into())
            })?;
            let config = config.clone().ok_or_else(|| {
                ApiError::Conflict("Migration restart has no executor configuration".into())
            })?;
            let session = Session::find_by_id(pool, reference_process.session_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("Migration session was not found".into()))?;
            let latest_session_info = CodingAgentTurn::find_latest_session_info(pool, session.id)
                .await?
                .ok_or_else(|| {
                    ApiError::Conflict("Migration session has no resumable agent identity".into())
                })?;
            let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
            let action = ExecutorAction::new(
                ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                    prompt: WORKSPACE_AFFINITY_MIGRATION_PROMPT.into(),
                    session_id: latest_session_info.session_id,
                    reset_to_message_id: None,
                    executor_config: config,
                    working_dir: session
                        .agent_working_dir
                        .as_ref()
                        .filter(|dir| !dir.is_empty())
                        .cloned(),
                }),
                deployment
                    .container()
                    .cleanup_actions_for_repos(&repos)
                    .map(Box::new),
            );
            deployment
                .container()
                .start_execution_with_id(
                    &workspace,
                    &session,
                    &action,
                    &ExecutionProcessRunReason::CodingAgent,
                    operation_id,
                )
                .await
                .map_err(ApiError::from)
        }
        .await;
        match restart {
            Ok(started) => Ok(WorkspaceAffinityUpdateResponse {
                placement: updated_placement,
                outcome: WorkspaceAffinityUpdateOutcome::Restarted,
                stopped_execution_id,
                started_execution: Some(started),
                message: None,
            }),
            Err(error) => Ok(WorkspaceAffinityUpdateResponse {
                placement: updated_placement,
                outcome: WorkspaceAffinityUpdateOutcome::RestartFailed,
                stopped_execution_id,
                started_execution: None,
                message: Some(format!(
                    "Affinity changed, but Vibe Kanban could not restart the task: {error}"
                )),
            }),
        }
    }
    .await;

    match result {
        Ok(response) => {
            finish_operation(pool, operation_id, &response).await?;
            Ok(ResponseJson(ApiResponse::success(response)))
        }
        Err(error) => {
            let recovery_source = sqlx::query_as::<_, (Option<Uuid>, bool)>(
                "SELECT source_execution_id, source_stop_started FROM workspace_affinity_operations WHERE operation_id = ?",
            )
            .bind(operation_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            let source_is_terminal = if let Some((Some(source_id), true)) = recovery_source {
                ExecutionProcess::find_by_id(pool, source_id)
                    .await
                    .ok()
                    .flatten()
                    .is_some_and(|source| {
                        !matches!(
                            source.status,
                            ExecutionProcessStatus::Running | ExecutionProcessStatus::Indeterminate
                        )
                    })
            } else {
                false
            };
            if !source_is_terminal {
                fail_operation(pool, operation_id, &error).await;
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::{AffinityIntent, UpdateWorkspaceAffinityRequest, operation_matches_request};

    fn request(
        run_on_coordinator: bool,
        requested_worker_node_id: Option<Uuid>,
    ) -> UpdateWorkspaceAffinityRequest {
        UpdateWorkspaceAffinityRequest {
            run_on_coordinator,
            requested_worker_node_id,
            restart_running: false,
            operation_id: None,
        }
    }

    #[test]
    fn affinity_intent_keeps_automatic_coordinator_and_worker_distinct() {
        let worker_id = Uuid::new_v4();

        assert_eq!(
            AffinityIntent::resolve(&request(false, None)).unwrap(),
            AffinityIntent::Automatic
        );
        assert_eq!(
            AffinityIntent::resolve(&request(true, None)).unwrap(),
            AffinityIntent::Coordinator
        );
        assert_eq!(
            AffinityIntent::resolve(&request(false, Some(worker_id))).unwrap(),
            AffinityIntent::Worker(worker_id)
        );
        assert!(AffinityIntent::resolve(&request(true, Some(worker_id))).is_err());
    }

    #[test]
    fn durable_operation_identity_distinguishes_coordinator_from_automatic() {
        let automatic = request(false, None);
        let coordinator = request(true, None);

        assert!(operation_matches_request(None, false, false, &automatic));
        assert!(!operation_matches_request(None, false, false, &coordinator));
        assert!(operation_matches_request(None, true, false, &coordinator));
        assert!(!operation_matches_request(None, true, false, &automatic));
    }
}
