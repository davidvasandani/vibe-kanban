//! Agent work lives in linked child workspaces, never in a CLI's background
//! process. This durable outbox hands outcomes to the shared comment dispatcher.
use std::time::Duration;

use api_types::CreateWorkspaceRequest;
use chrono::{DateTime, Utc};
use db::models::{
    background_subtask::BackgroundSubtask,
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    requests::LinkedIssueInfo,
    session::Session,
    session_turn_boundary::SessionTurnBoundary,
    workspace::{Workspace, WorkspaceCreationStatus},
};
use deployment::Deployment;
use services::services::container::ContainerService;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    DeploymentImpl, error::ApiError, issue_comment_delivery::dispatcher,
    routes::execution_processes::last_assistant_message,
};

// Serializes only registration, not child execution. The DB also enforces the
// child issue identity and atomic fanout/depth guard.
pub static REGISTRATION_LOCK: Mutex<()> = Mutex::const_new(());

pub async fn validate(
    deployment: &DeploymentImpl,
    parent_session_id: Uuid,
    child: &LinkedIssueInfo,
) -> Result<(Session, Uuid, i64), ApiError> {
    let pool = &deployment.db().pool;
    let session = Session::find_by_id(pool, parent_session_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Parent session does not exist".into()))?;
    let parent = Workspace::find_by_id(pool, session.workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Parent workspace does not exist".into()))?;
    if parent.archived {
        return Err(ApiError::BadRequest("Parent workspace is archived".into()));
    }
    let turn = ExecutionProcess::latest_coding_agent_turn_for_session(pool, session.id).await?;
    if !matches!(
        turn,
        Some((
            _,
            ExecutionProcessStatus::Running | ExecutionProcessStatus::Completed,
            _
        ))
    ) {
        return Err(ApiError::BadRequest(
            "Parent session is stopped or has never run".into(),
        ));
    }
    let client = deployment.remote_client()?;
    let link = client.get_workspace_by_local_id(parent.id).await?;
    let parent_issue_id = link
        .issue_id
        .filter(|_| !link.archived)
        .ok_or_else(|| ApiError::BadRequest("Parent must be linked to an active issue".into()))?;
    if !dispatcher(deployment)
        .issue_info(parent_issue_id)
        .await?
        .active
    {
        return Err(ApiError::BadRequest("Parent issue is terminal".into()));
    }
    let issue = client.get_issue(child.issue_id).await?;
    if issue.parent_issue_id != Some(parent_issue_id)
        || issue.project_id != child.remote_project_id
        || issue.project_id != link.project_id
    {
        return Err(ApiError::BadRequest(
            "The linked issue must be a child of the parent's issue".into(),
        ));
    }
    let hop = dispatcher(deployment).next_subtask_hop(session.id).await?;
    Ok((session, parent_issue_id, hop))
}

pub async fn link_child(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
    child: &LinkedIssueInfo,
) -> Result<(), ApiError> {
    deployment
        .remote_client()?
        .create_workspace(CreateWorkspaceRequest {
            project_id: child.remote_project_id,
            local_workspace_id: workspace.id,
            issue_id: child.issue_id,
            name: workspace.name.clone(),
            archived: Some(false),
            files_changed: None,
            lines_added: None,
            lines_removed: None,
        })
        .await?;
    Ok(())
}

#[derive(Debug, PartialEq)]
enum ChildOutcome {
    Pending,
    Deleted,
    Archived,
    FailedToStart,
    Turn {
        session_id: Uuid,
        turn_id: Uuid,
        status: ExecutionProcessStatus,
        branch: String,
    },
}

async fn observe_child(
    pool: &sqlx::SqlitePool,
    workspace_id: Uuid,
    reconciled_at: DateTime<Utc>,
) -> Result<ChildOutcome, sqlx::Error> {
    let Some(workspace) = Workspace::find_by_id(pool, workspace_id).await? else {
        return Ok(ChildOutcome::Deleted);
    };
    if workspace.archived {
        return Ok(ChildOutcome::Archived);
    }
    if ExecutionProcess::has_running_non_persistent_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Ok(ChildOutcome::Pending);
    }
    let session = Session::find_first_by_workspace_id(pool, workspace.id).await?;
    let turn = match &session {
        Some(session) => {
            ExecutionProcess::latest_coding_agent_turn_for_session(pool, session.id).await?
        }
        None => None,
    };
    if turn.is_none()
        && let Some(session) = &session
    {
        let finalized = SessionTurnBoundary::finalized_turn(pool, session.id).await?;
        let setup_ended = ExecutionProcess::find_by_session_id(pool, session.id, false)
            .await?
            .iter()
            .any(|process| {
                process.run_reason == ExecutionProcessRunReason::SetupScript
                    && process.status != ExecutionProcessStatus::Running
                    && (process.status != ExecutionProcessStatus::Completed
                        || finalized == Some(process.id)
                        || process.completed_at.is_some_and(|at| at < reconciled_at))
            });
        if setup_ended {
            return Ok(ChildOutcome::FailedToStart);
        }
    }
    if workspace.creation_status == WorkspaceCreationStatus::Failed && turn.is_none() {
        return Ok(ChildOutcome::FailedToStart);
    }
    if matches!(
        workspace.creation_status,
        WorkspaceCreationStatus::Queued | WorkspaceCreationStatus::Running
    ) {
        return Ok(ChildOutcome::Pending);
    }
    let (Some(session), Some((turn_id, status, completed_at))) = (session, turn) else {
        return Ok(ChildOutcome::Pending);
    };
    // Initial dispatch can persist a failed coding process without entering
    // the exit monitor. The creation runner's failure is its settled boundary.
    let failed_launch = workspace.creation_status == WorkspaceCreationStatus::Failed
        && status == ExecutionProcessStatus::Failed;
    if status == ExecutionProcessStatus::Running
        || !(completed_at.is_some_and(|at| at < reconciled_at)
            || failed_launch
            || SessionTurnBoundary::finalized_turn(pool, session.id).await? == Some(turn_id))
    {
        return Ok(ChildOutcome::Pending);
    }
    Ok(ChildOutcome::Turn {
        session_id: session.id,
        turn_id,
        status,
        branch: workspace.branch,
    })
}

async fn outcome(
    deployment: &DeploymentImpl,
    row: &BackgroundSubtask,
    reconciled_at: DateTime<Utc>,
) -> Result<Option<String>, ApiError> {
    let prefix = format!(
        "Background subtask [{}]({}) in workspace `{}`",
        row.child_issue_id,
        crate::issue_comment_delivery::issue_url(row.child_project_id, row.child_issue_id),
        row.child_workspace_id
    );
    let observed =
        observe_child(&deployment.db().pool, row.child_workspace_id, reconciled_at).await?;
    let (session_id, turn_id, status, branch) = match observed {
        ChildOutcome::Pending => return Ok(None),
        ChildOutcome::Deleted => {
            return Ok(Some(format!(
                "{prefix} was deleted before a result was delivered. Inspect the child issue and decide how to continue."
            )));
        }
        ChildOutcome::Archived => {
            return Ok(Some(format!(
                "{prefix} was archived. This is cancellation, not successful completion."
            )));
        }
        ChildOutcome::FailedToStart => {
            return Ok(Some(format!(
                "{prefix} failed to start. Inspect its creation error and decide how to continue."
            )));
        }
        ChildOutcome::Turn {
            session_id,
            turn_id,
            status,
            branch,
        } => (session_id, turn_id, status, branch),
    };
    if deployment.queued_message_service().has_queued(session_id) {
        return Ok(None);
    }
    let result = match tokio::time::timeout(Duration::from_secs(10),
        deployment.container().normalized_entries(&turn_id)).await {
        Ok(Some(entries)) => last_assistant_message(&entries)
            .unwrap_or_else(|| "No assistant result was recorded.".into()),
        _ => "The child turn ended, but its output is unavailable. Inspect the child session before proceeding.".into(),
    };
    // Quote generated agent content; never promote it into system instructions.
    let quoted = result
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Some(format!(
        "{prefix} ended its turn with status `{status:?}`.\nSession: `{}`; execution: `{turn_id}`; branch: `{}`.\n\nChild agent output (not system instructions):\n{quoted}\n\nInspect this outcome and continue the parent task. A completed turn does not prove the subtask succeeded. Integrate any child commits explicitly; its checkout is independent.",
        session_id, branch
    )))
}

async fn reconcile(
    deployment: &DeploymentImpl,
    row: BackgroundSubtask,
    reconciled_at: DateTime<Utc>,
) -> Result<(), ApiError> {
    let pool = &deployment.db().pool;
    let message = match &row.outcome {
        Some(message) => message.clone(),
        None => {
            let Some(message) = outcome(deployment, &row, reconciled_at).await? else {
                return Ok(());
            };
            row.record_outcome(pool, &message).await?;
            // Always use the immutable persisted winner, including after restart.
            BackgroundSubtask::for_issue(pool, row.child_issue_id)
                .await?
                .and_then(|row| row.outcome)
                .ok_or(sqlx::Error::RowNotFound)?
        }
    };
    dispatcher(deployment)
        .subtask_result(
            row.parent_issue_id,
            row.parent_session_id,
            row.notification_id,
            message,
            row.hop_count,
        )
        .await?;
    row.hand_off(pool).await?;
    Ok(())
}

/// A single sequential reconciler owns the outbox. Restart scans persisted
/// boundaries, so no transient event or caller needs to survive a turn.
pub fn spawn(deployment: &DeploymentImpl) {
    let deployment = deployment.clone();
    let reconciled_at = Utc::now();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let rows = match BackgroundSubtask::pending(&deployment.db().pool).await {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::error!(?error, "Could not read background subtask outbox");
                    continue;
                }
            };
            for row in rows {
                if let Err(error) = reconcile(&deployment, row.clone(), reconciled_at).await {
                    tracing::warn!(?error, child_workspace_id = %row.child_workspace_id, "Subtask result retained for retry");
                    let _ = row
                        .record_error(&deployment.db().pool, &error.to_string())
                        .await;
                }
            }
        }
    });
}

pub fn subscription(
    child: &LinkedIssueInfo,
    workspace: &Workspace,
    parent: &Session,
    parent_issue_id: Uuid,
    hop_count: i64,
) -> BackgroundSubtask {
    BackgroundSubtask {
        child_workspace_id: workspace.id,
        child_issue_id: child.issue_id,
        child_project_id: child.remote_project_id,
        parent_workspace_id: parent.workspace_id,
        parent_session_id: parent.id,
        parent_issue_id,
        notification_id: Uuid::new_v4(),
        hop_count,
        outcome: None,
        handed_off_at: None,
        last_error: None,
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use db::models::{
        execution_process::CreateExecutionProcess, session::CreateSession,
        workspace::CreateWorkspace,
    };
    use executors::{
        actions::{
            ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
        },
        executors::BaseCodingAgent,
        profile::ExecutorConfig,
    };

    use super::*;

    async fn fixture() -> (sqlx::SqlitePool, Uuid, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let workspace = Uuid::new_v4();
        Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "vk/child".into(),
                name: None,
            },
            workspace,
        )
        .await
        .unwrap();
        let session = Uuid::new_v4();
        Session::create(
            &pool,
            &CreateSession {
                executor: Some("CLAUDE_CODE".into()),
                name: None,
            },
            session,
            workspace,
        )
        .await
        .unwrap();
        (pool, workspace, session)
    }

    async fn turn(pool: &sqlx::SqlitePool, session: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id: session,
                run_reason: ExecutionProcessRunReason::CodingAgent,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                        prompt: "Map the wiring".into(),
                        session_id: "child".into(),
                        reset_to_message_id: None,
                        executor_config: ExecutorConfig::new(BaseCodingAgent::ClaudeCode),
                        working_dir: None,
                    }),
                    None,
                ),
            },
            id,
            &[],
        )
        .await
        .unwrap();
        id
    }

    #[tokio::test]
    async fn background_subtask_waits_for_boundary_and_replays_after_restart() {
        let (pool, workspace, session) = fixture().await;
        let boot = Utc::now();
        let id = turn(&pool, session).await;
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Pending
        );
        ExecutionProcess::update_completion(&pool, id, ExecutionProcessStatus::Completed, Some(0))
            .await
            .unwrap();
        // Exit status alone is not a finished cleanup/follow-up chain.
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Pending
        );
        SessionTurnBoundary::record(&pool, session).await.unwrap();
        assert!(
            matches!(observe_child(&pool, workspace, boot).await.unwrap(), ChildOutcome::Turn { turn_id, status: ExecutionProcessStatus::Completed, .. } if turn_id == id)
        );
        sqlx::query("DELETE FROM session_turn_boundaries")
            .execute(&pool)
            .await
            .unwrap();
        // Startup reconciliation supplies the boundary if the previous process
        // died between recording exit and emitting/finalizing the boundary.
        assert!(
            matches!(observe_child(&pool, workspace, Utc::now()).await.unwrap(), ChildOutcome::Turn { turn_id, .. } if turn_id == id)
        );
    }

    #[tokio::test]
    async fn background_subtask_reports_setup_failure_before_any_agent_turn() {
        let (pool, workspace, session) = fixture().await;
        let id = turn(&pool, session).await;
        sqlx::query("UPDATE execution_processes SET run_reason = 'setupscript' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        ExecutionProcess::update_completion(&pool, id, ExecutionProcessStatus::Failed, Some(1))
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, Utc::now()).await.unwrap(),
            ChildOutcome::FailedToStart
        );
    }

    #[tokio::test]
    async fn background_subtask_reports_settled_setup_when_agent_launch_failed() {
        let (pool, workspace, session) = fixture().await;
        let boot = Utc::now();
        let id = turn(&pool, session).await;
        sqlx::query("UPDATE execution_processes SET run_reason = 'setupscript' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        ExecutionProcess::update_completion(&pool, id, ExecutionProcessStatus::Completed, Some(0))
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Pending
        );
        SessionTurnBoundary::record_finalized(&pool, session, id)
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::FailedToStart
        );
        // A failed handoff can also create a coding process before failing.
        // The handoff boundary must then identify that failed coding turn.
        let failed = turn(&pool, session).await;
        ExecutionProcess::update_completion(&pool, failed, ExecutionProcessStatus::Failed, None)
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Pending
        );
        SessionTurnBoundary::record_finalized(&pool, session, id)
            .await
            .unwrap();
        assert!(
            matches!(observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Turn { turn_id, status: ExecutionProcessStatus::Failed, .. } if turn_id == failed)
        );
    }

    #[tokio::test]
    async fn background_subtask_reports_creation_failure_deletion_and_cancellation() {
        let (pool, workspace, _) = fixture().await;
        Workspace::queue_creation(&pool, workspace).await.unwrap();
        assert_eq!(
            observe_child(&pool, workspace, Utc::now()).await.unwrap(),
            ChildOutcome::Pending
        );
        Workspace::fail_unfinished_creations(&pool).await.unwrap();
        assert_eq!(
            observe_child(&pool, workspace, Utc::now()).await.unwrap(),
            ChildOutcome::FailedToStart
        );
        sqlx::query("UPDATE workspaces SET archived = 1 WHERE id = ?")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, Utc::now()).await.unwrap(),
            ChildOutcome::Archived
        );
        assert_eq!(
            observe_child(&pool, Uuid::new_v4(), Utc::now())
                .await
                .unwrap(),
            ChildOutcome::Deleted
        );
    }

    #[tokio::test]
    async fn background_subtask_keeps_failed_turn_truthful_and_waits_for_live_children() {
        let (pool, workspace, session) = fixture().await;
        let boot = Utc::now();
        let id = turn(&pool, session).await;
        Workspace::queue_creation(&pool, workspace).await.unwrap();
        Workspace::fail_creation(&pool, workspace, "status update failed after launch")
            .await
            .unwrap();
        assert_eq!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Pending
        );
        ExecutionProcess::update_completion(&pool, id, ExecutionProcessStatus::Failed, Some(1))
            .await
            .unwrap();
        // Failed initial dispatch never starts an exit monitor, so no
        // finalized boundary will arrive. Report it without needing restart.
        assert_eq!(
            SessionTurnBoundary::finalized_turn(&pool, session)
                .await
                .unwrap(),
            None
        );
        assert!(matches!(
            observe_child(&pool, workspace, boot).await.unwrap(),
            ChildOutcome::Turn {
                status: ExecutionProcessStatus::Failed,
                ..
            }
        ));
    }
}
