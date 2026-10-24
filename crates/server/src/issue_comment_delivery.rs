//! Delivery of issue comments to linked workspace sessions: the one path the
//! UI and the MCP share (VK constitution LVII).
//!
//! A comment lives on the remote server; sessions live in this server's
//! SQLite. A request is turned into a durable `issue_comment_deliveries` row
//! (unique per comment), resolved to exactly one session, and dispatched as a
//! normal follow-up turn at the session's turn boundary:
//!
//! - **Waiting** session (last turn completed): resumed immediately.
//! - **Running** session (or a queued follow-up, or a script still running):
//!   the row stays `pending` until `ContainerService::finalize_task` announces
//!   the turn boundary. It never starts a competing execution.
//! - **Stopped** turn, archived workspace, terminal issue, ambiguous target:
//!   recorded with a reason the sender sees; nothing is started.
//!
//! The dispatcher is the single owner of a delivery: it claims a row with a
//! conditional update before starting anything, and checks for an execution
//! already carrying the comment's marker (positive evidence of an earlier
//! start whose response was lost) so retries and restarts never duplicate a
//! turn. A 15 s sweep covers missed events, scheduled retries and restarts.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use api_types::{
    CreateIssueCommentRequest, IssueComment, ListIssueWorkspacesResponse,
    Workspace as RemoteWorkspace,
};
use axum::{Extension, Json, extract::State};
use chrono::{DateTime, Utc};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    issue_comment_delivery::{
        IssueCommentDelivery, IssueCommentDeliveryReason, IssueCommentDeliveryResolution,
        IssueCommentDeliverySource, IssueCommentDeliveryState, NewIssueCommentDelivery,
    },
    session::Session,
    session_turn_boundary::SessionTurnBoundary,
    workspace::Workspace,
};
use deployment::Deployment;
use executors::{actions::ExecutorActionType, profile::ExecutorConfig};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use services::services::{
    container::ContainerService,
    error_remediation::is_active_status_name,
    issue_comment_delivery::{
        AgentState, DeliveredComment, SessionCandidate, TargetDecision, compose_delivery_prompt,
        default_reply_session, derive_agent_state, next_hop_count, retry_delay,
        stored_marker_needle,
    },
    remote_client::RemoteClientError,
};
use sqlx::SqlitePool;
use tokio::sync::{Mutex, broadcast::error::RecvError};
use ts_rs::TS;
use uuid::Uuid;

use crate::{DeploymentImpl, routes::sessions::CreateFollowUpAttempt};

/// How often pending deliveries are re-examined (missed events, retries).
const SWEEP_INTERVAL: Duration = Duration::from_secs(5);
// A new turn may start only at a confirmed turn boundary: the session's
// latest turn is the one `finalize_task` recorded in `session_turn_boundaries`
// (written once the exit monitor has decided not to start a queued
// follow-up), or it ended before this process started (no exit monitor of
// an earlier process survives a restart, and its in-memory queue is gone).
// Elapsed time never authorizes a dispatch.

// ── Shared API types ────────────────────────────────────────────────────

/// A session that could receive comments on an issue, with its agent state.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DeliveryTargetCandidate {
    pub workspace_id: Uuid,
    pub workspace_name: Option<String>,
    pub workspace_branch: String,
    pub session_id: Uuid,
    pub session_name: Option<String>,
    pub agent_state: AgentState,
    pub latest_execution_id: Option<Uuid>,
}

/// Where a reply on an issue could go, from this host's point of view.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueAgentTargets {
    pub issue_id: Uuid,
    /// False when the issue is Done/Cancelled (nothing is resumed).
    pub issue_active: bool,
    pub candidates: Vec<DeliveryTargetCandidate>,
    /// Set only when exactly one session would receive a reply and it is
    /// waiting or running; the UI defaults to "reply" only then.
    pub default_session_id: Option<Uuid>,
    /// Why no session is eligible, when none is.
    pub unavailable_reason: Option<IssueCommentDeliveryReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueCommentWithDelivery {
    pub comment: IssueComment,
    pub author_name: Option<String>,
    pub delivery: Option<IssueCommentDelivery>,
    /// The comment changed after the agent received it; the agent saw the
    /// original (`delivery.delivered_message`).
    pub edited_after_delivery: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueCommentsWithDelivery {
    pub issue_id: Uuid,
    /// Browser route of the issue, when the issue could be read.
    pub issue_url: Option<String>,
    pub comments: Vec<IssueCommentWithDelivery>,
    /// Deliveries whose comment no longer exists on the issue.
    pub deleted_deliveries: Vec<IssueCommentDelivery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct LinkedIssueSummary {
    pub id: Uuid,
    pub simple_id: String,
    pub title: String,
    pub project_id: Uuid,
    /// Browser route of the issue (shared by the local and remote apps).
    pub issue_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceIssueComments {
    pub workspace_id: Uuid,
    /// `None` when the workspace is not linked to an issue.
    pub issue: Option<LinkedIssueSummary>,
    pub comments: Vec<IssueCommentWithDelivery>,
    pub deleted_deliveries: Vec<IssueCommentDelivery>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RequestIssueCommentDelivery {
    pub comment_id: Uuid,
    pub issue_id: Uuid,
    /// An explicit target; also consent to resume a stopped turn.
    #[serde(default)]
    pub target_session_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub source: Option<IssueCommentDeliverySource>,
    /// Session of the agent posting the comment (MCP), for loop guards.
    #[serde(default)]
    #[ts(optional)]
    pub origin_session_id: Option<Uuid>,
    /// Workspace of the agent posting the comment, when its session is unknown.
    #[serde(default)]
    #[ts(optional)]
    pub origin_workspace_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum IssueCommentIntent {
    /// Saved on the issue only; never sent to an agent.
    Note,
    /// Delivered to the linked session: resumes it if waiting, queued for
    /// its turn boundary if running.
    ReplyToAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateIssueCommentWithIntent {
    pub issue_id: Uuid,
    pub message: String,
    pub intent: IssueCommentIntent,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub target_session_id: Option<Uuid>,
    /// Idempotency key: retrying with the same id never creates a second
    /// comment or delivery.
    #[serde(default)]
    pub comment_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub origin_session_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub origin_workspace_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateIssueCommentResult {
    pub comment: IssueComment,
    /// Browser route of the issue; `None` if the issue could not be read
    /// after the comment was saved (the comment and delivery still stand).
    pub issue_url: Option<String>,
    /// `None` for notes: saved on the issue, not sent to any agent.
    pub delivery: Option<IssueCommentDelivery>,
}

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    #[error(transparent)]
    Remote(#[from] RemoteClientError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
}

impl From<DeliveryError> for crate::error::ApiError {
    fn from(error: DeliveryError) -> Self {
        match error {
            DeliveryError::Remote(error) => Self::RemoteClient(error),
            DeliveryError::Database(error) => Self::Database(error),
            DeliveryError::BadRequest(message) => Self::BadRequest(message),
            DeliveryError::Conflict(message) => Self::Conflict(message),
        }
    }
}

fn is_status(error: &RemoteClientError, code: u16) -> bool {
    matches!(error, RemoteClientError::Http { status, .. } if *status == code)
}

// ── Backend seam (remote server + execution start) ──────────────────────

#[derive(Debug, Clone)]
pub struct IssueInfo {
    pub id: Uuid,
    pub simple_id: String,
    pub title: String,
    pub project_id: Uuid,
    /// False when the issue's status is Done/Cancelled.
    pub active: bool,
}

/// Everything the dispatcher needs that is not local SQLite. Implemented by
/// the deployment, and by a fake in tests.
pub trait DeliveryBackend: Send + Sync + 'static {
    fn get_comment(
        &self,
        comment_id: Uuid,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>>;
    fn list_comments(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<Vec<IssueComment>, RemoteClientError>>;
    fn create_comment(
        &self,
        request: CreateIssueCommentRequest,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>>;
    fn issue_workspaces(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<ListIssueWorkspacesResponse, RemoteClientError>>;
    fn workspace_link(
        &self,
        local_workspace_id: Uuid,
    ) -> BoxFuture<'_, Result<Option<RemoteWorkspace>, RemoteClientError>>;
    fn issue_info(&self, issue_id: Uuid) -> BoxFuture<'_, Result<IssueInfo, RemoteClientError>>;
    /// Display names of project members (best effort; empty on failure).
    fn member_names(&self, project_id: Uuid) -> BoxFuture<'_, HashMap<Uuid, String>>;
    fn has_queued_message(&self, session_id: Uuid) -> bool;
    /// Start a follow-up turn in `session_id` carrying `prompt`; returns the
    /// new execution id or the backend's error message.
    fn start_follow_up(
        &self,
        session_id: Uuid,
        prompt: String,
    ) -> BoxFuture<'_, Result<Uuid, String>>;
}

/// The real backend: the signed-in remote client and the local follow-up path.
#[derive(Clone)]
pub struct DeploymentBackend {
    deployment: DeploymentImpl,
}

impl DeploymentBackend {
    fn client(&self) -> Result<services::services::remote_client::RemoteClient, RemoteClientError> {
        self.deployment
            .remote_client()
            .map_err(|_| RemoteClientError::Transport("remote server is not configured".into()))
    }
}

/// The executor configuration of the session's latest coding-agent turn, so a
/// delivered comment continues with the same agent, model and settings.
async fn latest_executor_config(
    pool: &SqlitePool,
    session: &Session,
) -> Result<ExecutorConfig, String> {
    let processes = ExecutionProcess::find_by_session_id(pool, session.id, false)
        .await
        .map_err(|e| format!("failed to load session executions: {e}"))?;
    processes
        .iter()
        .rev()
        .filter(|p| p.run_reason == ExecutionProcessRunReason::CodingAgent)
        .find_map(|p| match &p.executor_action().ok()?.typ {
            ExecutorActionType::CodingAgentInitialRequest(r) => Some(r.executor_config.clone()),
            ExecutorActionType::CodingAgentFollowUpRequest(r) => Some(r.executor_config.clone()),
            ExecutorActionType::ReviewRequest(r) => Some(r.executor_config.clone()),
            _ => None,
        })
        .ok_or_else(|| "session has no coding-agent turn to continue".to_string())
}

impl DeliveryBackend for DeploymentBackend {
    fn get_comment(
        &self,
        comment_id: Uuid,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        Box::pin(async move { self.client()?.get_issue_comment(comment_id).await })
    }

    fn list_comments(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<Vec<IssueComment>, RemoteClientError>> {
        Box::pin(async move {
            Ok(self
                .client()?
                .list_issue_comments(issue_id)
                .await?
                .issue_comments)
        })
    }

    fn create_comment(
        &self,
        request: CreateIssueCommentRequest,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        Box::pin(async move { Ok(self.client()?.create_issue_comment(&request).await?.data) })
    }

    fn issue_workspaces(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<ListIssueWorkspacesResponse, RemoteClientError>> {
        Box::pin(async move { self.client()?.list_issue_workspaces(issue_id).await })
    }

    fn workspace_link(
        &self,
        local_workspace_id: Uuid,
    ) -> BoxFuture<'_, Result<Option<RemoteWorkspace>, RemoteClientError>> {
        Box::pin(async move {
            match self
                .client()?
                .get_workspace_by_local_id(local_workspace_id)
                .await
            {
                Ok(workspace) => Ok(Some(workspace)),
                Err(error) if is_status(&error, 404) => Ok(None),
                Err(error) => Err(error),
            }
        })
    }

    fn issue_info(&self, issue_id: Uuid) -> BoxFuture<'_, Result<IssueInfo, RemoteClientError>> {
        Box::pin(async move {
            let client = self.client()?;
            let issue = client.get_issue(issue_id).await?;
            let statuses = client.list_project_statuses(issue.project_id).await?;
            // Fail closed: a status that cannot be found is not proof the
            // issue is active.
            let status = statuses
                .project_statuses
                .iter()
                .find(|status| status.id == issue.status_id)
                .ok_or_else(|| RemoteClientError::Http {
                    status: 502,
                    body: format!("status {} of issue {} not found", issue.status_id, issue.id),
                })?;
            Ok(IssueInfo {
                id: issue.id,
                simple_id: issue.simple_id,
                title: issue.title,
                project_id: issue.project_id,
                active: is_active_status_name(&status.name),
            })
        })
    }

    fn member_names(&self, project_id: Uuid) -> BoxFuture<'_, HashMap<Uuid, String>> {
        Box::pin(async move {
            let Ok(client) = self.client() else {
                return HashMap::new();
            };
            let Ok(project) = client.get_remote_project(project_id).await else {
                return HashMap::new();
            };
            let Ok(members) = client.list_members(project.organization_id).await else {
                return HashMap::new();
            };
            members
                .members
                .into_iter()
                .filter_map(|member| {
                    let full = format!(
                        "{} {}",
                        member.first_name.unwrap_or_default(),
                        member.last_name.unwrap_or_default()
                    );
                    let name = Some(full.trim().to_string())
                        .filter(|name| !name.is_empty())
                        .or(member.username)
                        .or(member.email)?;
                    Some((member.user_id, name))
                })
                .collect()
        })
    }

    fn has_queued_message(&self, session_id: Uuid) -> bool {
        self.deployment
            .queued_message_service()
            .has_queued(session_id)
    }

    fn start_follow_up(
        &self,
        session_id: Uuid,
        prompt: String,
    ) -> BoxFuture<'_, Result<Uuid, String>> {
        Box::pin(async move {
            let pool = &self.deployment.db().pool;
            let session = Session::find_by_id(pool, session_id)
                .await
                .map_err(|e| format!("failed to load session {session_id}: {e}"))?
                .ok_or_else(|| format!("session {session_id} no longer exists"))?;
            let executor_config = latest_executor_config(pool, &session).await?;
            // The same path as a typed follow-up and MCP `run_session_prompt`:
            // executor checks, container, cleanup action and activation.
            let response = crate::routes::sessions::follow_up(
                Extension(session),
                State(self.deployment.clone()),
                Json(CreateFollowUpAttempt {
                    prompt,
                    executor_config,
                    retry_process_id: None,
                    force_when_dirty: None,
                    perform_git_reset: None,
                }),
            )
            .await
            .map_err(|error| error.to_string())?;
            response
                .0
                .into_data()
                .map(|process| process.id)
                .ok_or_else(|| "follow-up returned no execution".to_string())
        })
    }
}

// ── The dispatcher ──────────────────────────────────────────────────────

enum EarlierAttempt {
    Launched(Uuid),
    Uncertain(Uuid),
    None,
}

pub struct IssueCommentDispatcher<B: DeliveryBackend> {
    pool: SqlitePool,
    backend: B,
    session_locks: StdMutex<HashMap<Uuid, Arc<Mutex<()>>>>,
    /// Turns that ended before this instant are boundaries: it starts as
    /// this dispatcher's creation and moves to the end of startup
    /// reconciliation (orphan cleanup marks interrupted turns without
    /// `finalize_task`, then boot decides whether to resume them).
    reconciled_at: StdMutex<DateTime<Utc>>,
}

/// One candidate with the presentation details the API reports.
struct ResolvedCandidates {
    issue: IssueInfo,
    candidates: Vec<DeliveryTargetCandidate>,
    unavailable_reason: Option<IssueCommentDeliveryReason>,
}

impl<B: DeliveryBackend> IssueCommentDispatcher<B> {
    pub fn new(pool: SqlitePool, backend: B) -> Self {
        Self {
            pool,
            backend,
            session_locks: StdMutex::new(HashMap::new()),
            reconciled_at: StdMutex::new(Utc::now()),
        }
    }

    fn session_lock(&self, session_id: Uuid) -> Arc<Mutex<()>> {
        self.session_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(session_id)
            .or_default()
            .clone()
    }

    async fn boundary_confirmed(
        &self,
        session_id: Uuid,
        turn_id: Uuid,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<bool, sqlx::Error> {
        let reconciled_at = *self
            .reconciled_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if completed_at.is_some_and(|at| at < reconciled_at) {
            return Ok(true);
        }
        Ok(SessionTurnBoundary::finalized_turn(&self.pool, session_id).await? == Some(turn_id))
    }

    async fn session_state(
        &self,
        session: &Session,
    ) -> Result<(AgentState, Option<Uuid>), sqlx::Error> {
        let latest =
            ExecutionProcess::latest_coding_agent_turn_for_session(&self.pool, session.id).await?;
        let running =
            ExecutionProcess::has_running_coding_agent_for_session(&self.pool, session.id).await?;
        let queued = self.backend.has_queued_message(session.id);
        Ok((
            derive_agent_state(
                latest.as_ref().map(|(_, status, _)| status),
                running,
                queued,
            ),
            latest.map(|(id, _, _)| id),
        ))
    }

    /// Sessions on this host that could receive comments on `issue_id`.
    async fn resolve_candidates(
        &self,
        issue_id: Uuid,
    ) -> Result<ResolvedCandidates, DeliveryError> {
        let issue = self.backend.issue_info(issue_id).await?;
        let links = self.backend.issue_workspaces(issue_id).await?;

        let mut candidates = Vec::new();
        let mut local_archived = 0usize;
        let mut other_host = 0usize;
        for link in &links.workspaces {
            let Some(local_id) = link.local_workspace_id else {
                other_host += 1;
                continue;
            };
            let Some(workspace) = Workspace::find_by_id(&self.pool, local_id).await? else {
                other_host += 1;
                continue;
            };
            if workspace.archived {
                local_archived += 1;
                continue;
            }
            for session in Session::find_by_workspace_id(&self.pool, workspace.id).await? {
                let (agent_state, latest_execution_id) = self.session_state(&session).await?;
                // Only sessions that have run an agent can be continued.
                if agent_state == AgentState::NeverRun {
                    continue;
                }
                candidates.push(DeliveryTargetCandidate {
                    workspace_id: workspace.id,
                    workspace_name: workspace.name.clone(),
                    workspace_branch: workspace.branch.clone(),
                    session_id: session.id,
                    session_name: session.name.clone(),
                    agent_state,
                    latest_execution_id,
                });
            }
        }

        let unavailable_reason = if !issue.active {
            Some(IssueCommentDeliveryReason::IssueTerminal)
        } else if !candidates.is_empty() {
            None
        } else if links.workspaces.is_empty() && links.archived_count == 0 {
            Some(IssueCommentDeliveryReason::NoLinkedWorkspace)
        } else if local_archived > 0 || (links.workspaces.is_empty() && links.archived_count > 0) {
            Some(IssueCommentDeliveryReason::WorkspaceArchived)
        } else if other_host > 0 {
            Some(IssueCommentDeliveryReason::WorkspaceOtherHost)
        } else {
            Some(IssueCommentDeliveryReason::NoSessions)
        };

        Ok(ResolvedCandidates {
            issue,
            candidates,
            unavailable_reason,
        })
    }

    /// `GET /api/issues/{issue_id}/agent-targets`.
    pub async fn agent_targets(&self, issue_id: Uuid) -> Result<IssueAgentTargets, DeliveryError> {
        let resolved = self.resolve_candidates(issue_id).await?;
        let default_session_id = if resolved.unavailable_reason.is_some() {
            None
        } else {
            default_reply_session(&to_session_candidates(&resolved.candidates))
        };
        Ok(IssueAgentTargets {
            issue_id,
            issue_active: resolved.issue.active,
            candidates: resolved.candidates,
            default_session_id,
            unavailable_reason: resolved.unavailable_reason,
        })
    }

    /// The session of the agent posting a comment: the one given, else the
    /// workspace's running coding agent (the caller is that agent).
    async fn origin_session(
        &self,
        origin_session_id: Option<Uuid>,
        origin_workspace_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        if origin_session_id.is_some() {
            return Ok(origin_session_id);
        }
        let Some(workspace_id) = origin_workspace_id else {
            return Ok(None);
        };
        Ok(
            ExecutionProcess::find_running_coding_agents_by_workspace(&self.pool, workspace_id)
                .await?
                .first()
                .map(|process| process.session_id),
        )
    }

    /// Hop count of a new agent-originated delivery: one past the delivery
    /// that started the origin session's current turn.
    async fn origin_hop(&self, origin_session_id: Uuid) -> Result<Option<i64>, sqlx::Error> {
        let Some((turn_id, _, _)) =
            ExecutionProcess::latest_coding_agent_turn_for_session(&self.pool, origin_session_id)
                .await?
        else {
            return Ok(None);
        };
        Ok(
            IssueCommentDelivery::find_by_execution_process_id(&self.pool, turn_id)
                .await?
                .map(|delivery| delivery.hop_count),
        )
    }

    /// Decide where a delivery goes. Remote failures become a visible,
    /// retryable `failed{remote_unavailable}`, never "no target".
    async fn resolve(
        &self,
        issue_id: Uuid,
        explicit_session_id: Option<Uuid>,
        origin_session_id: Option<Uuid>,
        agent_origin: bool,
    ) -> Result<IssueCommentDeliveryResolution, sqlx::Error> {
        let outcome = |state, reason, error: Option<String>| IssueCommentDeliveryResolution {
            state,
            reason: Some(reason),
            error,
            workspace_id: None,
            session_id: None,
            hop_count: 0,
            allow_stopped_target: false,
        };

        let origin_turn_hop = match origin_session_id {
            Some(origin) if agent_origin => self.origin_hop(origin).await?,
            _ => None,
        };
        let hop_count = match next_hop_count(agent_origin, origin_turn_hop) {
            Ok(hop) => hop,
            Err(reason) => return Ok(outcome(IssueCommentDeliveryState::Blocked, reason, None)),
        };

        let resolved = match self.resolve_candidates(issue_id).await {
            Ok(resolved) => resolved,
            Err(DeliveryError::Database(error)) => return Err(error),
            Err(error) => {
                let reason = match &error {
                    DeliveryError::Remote(remote) if is_status(remote, 403) => {
                        IssueCommentDeliveryReason::Unauthorized
                    }
                    _ => IssueCommentDeliveryReason::RemoteUnavailable,
                };
                return Ok(outcome(
                    IssueCommentDeliveryState::Failed,
                    reason,
                    Some(format!(
                        "could not resolve sessions linked to issue {issue_id}: {error}"
                    )),
                ));
            }
        };
        if let Some(reason) = resolved.unavailable_reason {
            return Ok(outcome(IssueCommentDeliveryState::NoTarget, reason, None));
        }

        let decision =
            select_target_from(&resolved.candidates, explicit_session_id, origin_session_id);
        Ok(match decision {
            TargetDecision::Target {
                candidate,
                explicit,
            } => IssueCommentDeliveryResolution {
                state: IssueCommentDeliveryState::Pending,
                reason: None,
                error: None,
                workspace_id: Some(candidate.workspace_id),
                session_id: Some(candidate.session_id),
                hop_count,
                allow_stopped_target: explicit,
            },
            TargetDecision::NeedsTarget => IssueCommentDeliveryResolution {
                hop_count,
                ..outcome(
                    IssueCommentDeliveryState::NeedsTarget,
                    IssueCommentDeliveryReason::Ambiguous,
                    None,
                )
            },
            TargetDecision::NoTarget(reason) => IssueCommentDeliveryResolution {
                hop_count,
                ..outcome(IssueCommentDeliveryState::NoTarget, reason, None)
            },
            TargetDecision::Blocked(reason) => {
                outcome(IssueCommentDeliveryState::Blocked, reason, None)
            }
        })
    }

    /// The shared delivery path: ask for an existing comment to reach a
    /// session. Idempotent per comment; a retry or target choice re-resolves
    /// a `failed`/`no_target`/`needs_target` delivery, anything else is
    /// returned as is.
    pub async fn request(
        &self,
        request: RequestIssueCommentDelivery,
    ) -> Result<IssueCommentDelivery, DeliveryError> {
        // Authoritative content and authorization: the signed-in user must be
        // able to read the comment on the remote server.
        let comment = self.backend.get_comment(request.comment_id).await?;
        if comment.issue_id != request.issue_id {
            return Err(DeliveryError::BadRequest(format!(
                "comment {} does not belong to issue {}",
                request.comment_id, request.issue_id
            )));
        }

        let source = request.source.unwrap_or(IssueCommentDeliverySource::Ui);

        if let Some(existing) =
            IssueCommentDelivery::find_by_comment_id(&self.pool, comment.id).await?
        {
            // Pending, dispatching, delivered, blocked or cancelled: a repeat
            // request changes nothing (and never starts a second turn).
            if !existing.state.is_resolvable() {
                return Ok(existing);
            }
            // A retry or target choice. Who posted was fixed at creation.
            let resolution = self
                .resolve(
                    existing.issue_id,
                    request.target_session_id,
                    existing.origin_session_id,
                    existing.source == IssueCommentDeliverySource::Mcp
                        && existing.origin_session_id.is_some(),
                )
                .await?;
            let applied = IssueCommentDelivery::apply_resolution(
                &self.pool,
                existing.id,
                &[
                    IssueCommentDeliveryState::Failed,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryState::NeedsTarget,
                ],
                &resolution,
                Utc::now(),
            )
            .await?;
            if applied {
                self.dispatch_if_pending(&resolution).await?;
            }
            return Ok(IssueCommentDelivery::find_by_id(&self.pool, existing.id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?);
        }

        // Resolve first, then insert the resolved row: a crash in between
        // leaves no row, so nothing can wait forever without a target.
        let origin_session_id = self
            .origin_session(request.origin_session_id, request.origin_workspace_id)
            .await?;
        let resolution = self
            .resolve(
                comment.issue_id,
                request.target_session_id,
                origin_session_id,
                source == IssueCommentDeliverySource::Mcp && origin_session_id.is_some(),
            )
            .await?;
        let (row, created) = IssueCommentDelivery::insert_if_absent(
            &self.pool,
            &NewIssueCommentDelivery {
                comment_id: comment.id,
                issue_id: comment.issue_id,
                source,
                origin_session_id,
                comment_author_id: comment.author_id,
                comment_created_at: comment.created_at,
            },
            &resolution,
            Utc::now(),
        )
        .await?;
        if created {
            self.dispatch_if_pending(&resolution).await?;
        }
        Ok(IssueCommentDelivery::find_by_id(&self.pool, row.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?)
    }

    async fn dispatch_if_pending(
        &self,
        resolution: &IssueCommentDeliveryResolution,
    ) -> Result<(), sqlx::Error> {
        if let (IssueCommentDeliveryState::Pending, Some(session_id)) =
            (resolution.state, resolution.session_id)
        {
            self.dispatch_session(session_id).await?;
        }
        Ok(())
    }

    /// Capture ancestry when accepting a child, rather than when it finishes:
    /// siblings spawned by one turn share a hop even if they finish apart.
    pub async fn next_subtask_hop(&self, parent_session: Uuid) -> Result<i64, DeliveryError> {
        next_hop_count(true, self.origin_hop(parent_session).await?).map_err(|_| {
            DeliveryError::BadRequest(
                "Background subtask continuation limit reached; continue in the foreground".into(),
            )
        })
    }

    /// Internal completion delivery pins one session without granting the
    /// user-only permission to revive a stopped turn. Replays never re-resolve
    /// terminal deliveries: an automatic notification is not a manual retry.
    pub async fn subtask_result(
        &self,
        issue_id: Uuid,
        session_id: Uuid,
        comment_id: Uuid,
        message: String,
        hop_count: i64,
    ) -> Result<IssueCommentDelivery, DeliveryError> {
        let (comment, _, _) = self
            .create_comment(CreateIssueCommentWithIntent {
                issue_id,
                message,
                intent: IssueCommentIntent::Note,
                parent_id: None,
                target_session_id: None,
                comment_id: Some(comment_id),
                origin_session_id: None,
                origin_workspace_id: None,
            })
            .await?;
        if let Some(existing) =
            IssueCommentDelivery::find_by_comment_id(&self.pool, comment_id).await?
        {
            return Ok(existing);
        }
        let mut resolution = self
            .resolve(issue_id, Some(session_id), None, false)
            .await?;
        if resolution.state == IssueCommentDeliveryState::Failed {
            return Err(DeliveryError::BadRequest(resolution.error.unwrap_or_else(
                || "Could not resolve the parent session; notification retained for retry".into(),
            )));
        }
        resolution.allow_stopped_target = false;
        resolution.hop_count = hop_count;
        if !(1..=services::services::issue_comment_delivery::MAX_AGENT_HOPS).contains(&hop_count) {
            resolution.state = IssueCommentDeliveryState::Blocked;
            resolution.reason = Some(IssueCommentDeliveryReason::LoopGuard);
        }
        let (row, created) = IssueCommentDelivery::insert_if_absent(
            &self.pool,
            &NewIssueCommentDelivery {
                comment_id,
                issue_id,
                source: IssueCommentDeliverySource::Mcp,
                origin_session_id: None,
                comment_author_id: comment.author_id,
                comment_created_at: comment.created_at,
            },
            &resolution,
            Utc::now(),
        )
        .await?;
        if created {
            self.dispatch_if_pending(&resolution).await?;
        }
        Ok(IssueCommentDelivery::find_by_id(&self.pool, row.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?)
    }

    /// MCP create: save the comment (idempotently, by `comment_id`) and, for
    /// a reply, deliver it through [`Self::request`].
    pub async fn create_comment(
        &self,
        request: CreateIssueCommentWithIntent,
    ) -> Result<
        (
            IssueComment,
            Option<IssueInfo>,
            Option<IssueCommentDelivery>,
        ),
        DeliveryError,
    > {
        let message = request.message.trim().to_string();
        if message.is_empty() {
            return Err(DeliveryError::BadRequest(
                "message must not be empty".into(),
            ));
        }
        let comment_id = request.comment_id.unwrap_or_else(Uuid::new_v4);
        let comment = match self
            .backend
            .create_comment(CreateIssueCommentRequest {
                id: Some(comment_id),
                issue_id: request.issue_id,
                message: message.clone(),
                parent_id: request.parent_id,
            })
            .await
        {
            Ok(comment) => comment,
            // A retry of a create whose response was lost: the comment exists.
            Err(error) if is_status(&error, 409) => {
                let existing = self.backend.get_comment(comment_id).await?;
                if existing.issue_id != request.issue_id || existing.message != message {
                    return Err(DeliveryError::Conflict(format!(
                        "comment id {comment_id} is already used by a different comment"
                    )));
                }
                existing
            }
            Err(error) => return Err(error.into()),
        };
        let delivery = match request.intent {
            IssueCommentIntent::Note => None,
            IssueCommentIntent::ReplyToAgent => Some(
                self.request(RequestIssueCommentDelivery {
                    comment_id: comment.id,
                    issue_id: comment.issue_id,
                    target_session_id: request.target_session_id,
                    source: Some(IssueCommentDeliverySource::Mcp),
                    origin_session_id: request.origin_session_id,
                    origin_workspace_id: request.origin_workspace_id,
                })
                .await?,
            ),
        };
        // Once the comment exists, failing the call would invite a retry
        // that cannot help; the URL is best effort.
        let issue = self.backend.issue_info(comment.issue_id).await.ok();
        Ok((comment, issue, delivery))
    }

    /// Deliver what is pending for one session, if it is at a turn boundary.
    async fn dispatch_session(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        // Held until the turn has started. This is deliberately different from
        // the usual claim-then-release (constitution XII): the row claim keeps
        // one row from being delivered twice, but only this per-session lock
        // stops a second batch from seeing "not running" in the gap before the
        // first batch's execution row exists, which would start a competing
        // turn. It is per session, and every awaited remote call has a client
        // timeout, so other sessions are never blocked.
        let lock = self.session_lock(session_id);
        let _guard = lock.lock().await;
        let now = Utc::now();

        let due: Vec<IssueCommentDelivery> =
            IssueCommentDelivery::list_pending_for_session(&self.pool, session_id)
                .await?
                .into_iter()
                .filter(|row| row.next_attempt_at.is_none_or(|at| at <= now))
                .collect();
        let mut eligible = Vec::new();
        for row in due {
            if let Some(wakeup) = db::models::wakeup::Wakeup::find(&self.pool, row.comment_id).await?
                && wakeup.receipt_path.is_some() && wakeup.trigger_context.is_none() {
                continue;
            }
            eligible.push(row);
        }
        let due = eligible;
        let Some(issue_id) = due.first().map(|row| row.issue_id) else {
            return Ok(());
        };

        let session = Session::find_by_id(&self.pool, session_id).await?;
        let workspace = match &session {
            Some(session) => Workspace::find_by_id(&self.pool, session.workspace_id).await?,
            None => None,
        };
        let (Some(session), Some(workspace)) = (session, workspace) else {
            for row in &due {
                IssueCommentDelivery::finish_pending(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryReason::NoSessions,
                    now,
                )
                .await?;
            }
            return Ok(());
        };
        if workspace.archived {
            for row in &due {
                IssueCommentDelivery::finish_pending(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryReason::WorkspaceArchived,
                    now,
                )
                .await?;
            }
            return Ok(());
        }

        // Busy: wait for the turn boundary. A queued follow-up goes first,
        // and setup/cleanup scripts finish their chain before a new turn.
        let busy_reason =
            if ExecutionProcess::has_running_coding_agent_for_session(&self.pool, session_id)
                .await?
            {
                Some(IssueCommentDeliveryReason::AgentRunning)
            } else if self.backend.has_queued_message(session_id) {
                Some(IssueCommentDeliveryReason::QueuedMessage)
            } else if ExecutionProcess::has_running_non_persistent_processes_for_workspace(
                &self.pool,
                workspace.id,
            )
            .await?
            {
                Some(IssueCommentDeliveryReason::AgentRunning)
            } else {
                None
            };
        if let Some(reason) = busy_reason {
            for row in &due {
                if row.reason != Some(reason)
                    && row.reason != Some(IssueCommentDeliveryReason::RetryScheduled)
                {
                    IssueCommentDelivery::set_pending_reason(&self.pool, row.id, reason, now)
                        .await?;
                }
            }
            return Ok(());
        }

        let latest =
            ExecutionProcess::latest_coding_agent_turn_for_session(&self.pool, session_id).await?;
        let Some((latest_turn_id, latest_status, completed_at)) = latest else {
            for row in &due {
                IssueCommentDelivery::finish_pending(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryReason::NoSessions,
                    now,
                )
                .await?;
            }
            return Ok(());
        };

        // The latest turn may be one of our own failed launches for this
        // batch: the agent did not stop, and nothing else is in flight.
        let mut own_attempt = false;
        for row in &due {
            if ExecutionProcess::find_coding_agent_turns_with_marker(
                &self.pool,
                session_id,
                &stored_marker_needle(row.comment_id),
            )
            .await?
            .iter()
            .any(|turn| {
                turn.id == latest_turn_id
                    && !turn.launched
                    && turn.status != ExecutionProcessStatus::Running
            }) {
                own_attempt = true;
                break;
            }
        }

        // Only at a confirmed boundary; the finalize event or the sweep comes
        // back otherwise.
        if !own_attempt
            && !self
                .boundary_confirmed(session_id, latest_turn_id, completed_at)
                .await?
        {
            return Ok(());
        }

        let stopped = latest_status != ExecutionProcessStatus::Completed && !own_attempt;
        let mut deliverable = Vec::new();
        for row in due.into_iter().filter(|row| row.issue_id == issue_id) {
            if stopped && !row.allow_stopped_target {
                // Implicit delivery never resumes a failed or stopped turn.
                IssueCommentDelivery::finish_pending(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryReason::TurnStopped,
                    now,
                )
                .await?;
            } else if IssueCommentDelivery::claim(&self.pool, row.id, now).await? {
                deliverable.push(row);
            }
        }
        if deliverable.is_empty() {
            return Ok(());
        }
        self.deliver_claimed(&session, deliverable).await
    }

    /// What the session's executions say about an earlier attempt to deliver
    /// `comment_id` (see `find_coding_agent_turns_with_marker`). A turn
    /// whose agent launched is delivered, whatever its status since: a turn
    /// that ran and then failed is never replayed. A launch that failed or
    /// was cut off before the agent started is no evidence, so it is sent
    /// again. A turn handed to a worker that never confirmed acceptance is
    /// unknown: it is neither delivered nor retried automatically.
    async fn earlier_attempt(
        &self,
        session_id: Uuid,
        comment_id: Uuid,
    ) -> Result<EarlierAttempt, sqlx::Error> {
        let turns = ExecutionProcess::find_coding_agent_turns_with_marker(
            &self.pool,
            session_id,
            &stored_marker_needle(comment_id),
        )
        .await?;
        if let Some(turn) = turns.iter().find(|turn| turn.launched) {
            return Ok(EarlierAttempt::Launched(turn.id));
        }
        Ok(turns
            .iter()
            .find(|turn| turn.uncertain)
            .map_or(EarlierAttempt::None, |turn| {
                EarlierAttempt::Uncertain(turn.id)
            }))
    }

    /// The most recent attempt carrying `comment_id`, classified the same way.
    async fn latest_attempt(
        &self,
        session_id: Uuid,
        comment_id: Uuid,
    ) -> Result<EarlierAttempt, sqlx::Error> {
        let turns = ExecutionProcess::find_coding_agent_turns_with_marker(
            &self.pool,
            session_id,
            &stored_marker_needle(comment_id),
        )
        .await?;
        Ok(match turns.last() {
            Some(turn) if turn.launched => EarlierAttempt::Launched(turn.id),
            Some(turn) if turn.uncertain => EarlierAttempt::Uncertain(turn.id),
            _ => EarlierAttempt::None,
        })
    }

    /// Record an unknown worker launch: visible, never auto-retried.
    async fn fail_uncertain(
        &self,
        row: &IssueCommentDelivery,
        execution_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        IssueCommentDelivery::finish_dispatch(
            &self.pool,
            row.id,
            IssueCommentDeliveryState::Failed,
            Some(IssueCommentDeliveryReason::LaunchUncertain),
            Some(&format!(
                "execution {execution_id} was handed to a worker that never confirmed it started; \
                 check the session, then choose it explicitly to send the comment again"
            )),
            None,
            Utc::now(),
        )
        .await?;
        Ok(())
    }

    /// Finish every claimed row of a batch with the same outcome.
    async fn finish_all(
        &self,
        rows: &[&IssueCommentDelivery],
        state: IssueCommentDeliveryState,
        reason: IssueCommentDeliveryReason,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        for row in rows {
            IssueCommentDelivery::finish_dispatch(
                &self.pool,
                row.id,
                state,
                Some(reason),
                None,
                None,
                now,
            )
            .await?;
        }
        Ok(())
    }

    /// Start one turn carrying every claimed comment (all on one issue).
    async fn deliver_claimed(
        &self,
        session: &Session,
        claimed: Vec<IssueCommentDelivery>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let mut ready: Vec<(IssueCommentDelivery, IssueComment)> = Vec::new();
        for row in claimed {
            // Positive evidence of an earlier start whose outcome was lost:
            // record it with the snapshot taken before that start.
            match self.earlier_attempt(session.id, row.comment_id).await? {
                EarlierAttempt::Launched(execution_id) => {
                    IssueCommentDelivery::mark_delivered(
                        &self.pool,
                        row.id,
                        execution_id,
                        row.delivered_message.as_deref().unwrap_or_default(),
                        row.delivered_comment_updated_at,
                        now,
                    )
                    .await?;
                    continue;
                }
                // An explicit choice is consent to send it again.
                EarlierAttempt::Uncertain(execution_id) if !row.allow_stopped_target => {
                    self.fail_uncertain(&row, execution_id).await?;
                    continue;
                }
                EarlierAttempt::Uncertain(_) | EarlierAttempt::None => {}
            }
            if let Some(wakeup) = db::models::wakeup::Wakeup::find(&self.pool, row.comment_id).await? {
                ready.push((row, IssueComment {
                    id: wakeup.id, issue_id: Uuid::nil(), author_id: None, parent_id: None,
                    message: wakeup.message(now), created_at: wakeup.created_at, updated_at: now,
                }));
                continue;
            }
            match self.backend.get_comment(row.comment_id).await {
                Ok(comment) => ready.push((row, comment)),
                Err(error) if is_status(&error, 404) => {
                    self.finish_all(
                        &[&row],
                        IssueCommentDeliveryState::Cancelled,
                        IssueCommentDeliveryReason::CommentDeleted,
                    )
                    .await?;
                }
                Err(error) if is_status(&error, 403) => {
                    IssueCommentDelivery::finish_dispatch(
                        &self.pool,
                        row.id,
                        IssueCommentDeliveryState::Failed,
                        Some(IssueCommentDeliveryReason::Unauthorized),
                        Some(&format!(
                            "not allowed to read comment {}: {error}",
                            row.comment_id
                        )),
                        None,
                        now,
                    )
                    .await?;
                }
                Err(error) => {
                    self.retry_or_fail(
                        &row,
                        IssueCommentDeliveryReason::RemoteUnavailable,
                        &format!("could not re-read comment {}: {error}", row.comment_id),
                    )
                    .await?;
                }
            }
        }
        if ready.is_empty() {
            return Ok(());
        }
        let rows: Vec<&IssueCommentDelivery> = ready.iter().map(|(row, _)| row).collect();
        let issue_id = ready[0].0.issue_id;

        let local_wakeup = issue_id.is_nil();
        let prompt = if local_wakeup {
            ready.iter().map(|(_, comment)| format!("{}\n<!-- {} -->", comment.message,
                services::services::issue_comment_delivery::delivery_marker(comment.id)))
                .collect::<Vec<_>>().join("\n\n")
        } else {
        // The target may have changed while the comment waited: the issue
        // closed, or the workspace was unlinked, relinked or archived.
        let (issue, link) = match (
            self.backend.issue_info(issue_id).await,
            self.backend.workspace_link(session.workspace_id).await,
        ) {
            (Ok(issue), Ok(link)) => (issue, link),
            (Err(error), _) | (_, Err(error)) => {
                for row in &rows {
                    self.retry_or_fail(
                        row,
                        IssueCommentDeliveryReason::RemoteUnavailable,
                        &format!(
                            "could not re-check issue {issue_id} and workspace {}: {error}",
                            session.workspace_id
                        ),
                    )
                    .await?;
                }
                return Ok(());
            }
        };
        if !issue.active {
            return self
                .finish_all(
                    &rows,
                    IssueCommentDeliveryState::NoTarget,
                    IssueCommentDeliveryReason::IssueTerminal,
                )
                .await;
        }
        match link {
            Some(link) if link.issue_id == Some(issue_id) && link.archived => {
                return self
                    .finish_all(
                        &rows,
                        IssueCommentDeliveryState::NoTarget,
                        IssueCommentDeliveryReason::WorkspaceArchived,
                    )
                    .await;
            }
            Some(link) if link.issue_id == Some(issue_id) => {}
            _ => {
                return self
                    .finish_all(
                        &rows,
                        IssueCommentDeliveryState::NoTarget,
                        IssueCommentDeliveryReason::TargetNotLinked,
                    )
                    .await;
            }
        }

        let names = self.backend.member_names(issue.project_id).await;
        let delivered: Vec<DeliveredComment> = ready
            .iter()
            .map(|(_, comment)| DeliveredComment {
                comment_id: comment.id,
                author_name: comment.author_id.and_then(|id| names.get(&id).cloned()),
                created_at: comment.created_at,
                message: comment.message.clone(),
            })
            .collect();
        let label = format!("{} (\"{}\")", issue.simple_id, issue.title);
        compose_delivery_prompt(&label, &delivered)

        };

        // The snapshot is durable before the turn can start, so recovery
        // after a crash records exactly what the agent received.
        for (row, comment) in &ready {
            IssueCommentDelivery::record_snapshot(
                &self.pool,
                row.id,
                &comment.message,
                comment.updated_at,
                Utc::now(),
            )
            .await?;
        }

        match self.backend.start_follow_up(session.id, prompt).await {
            Ok(execution_id) => {
                let now = Utc::now();
                for (row, comment) in &ready {
                    IssueCommentDelivery::mark_delivered(
                        &self.pool,
                        row.id,
                        execution_id,
                        &comment.message,
                        Some(comment.updated_at),
                        now,
                    )
                    .await?;
                }
                tracing::info!(
                    session_id = %session.id,
                    %execution_id,
                    comments = ready.len(),
                    "delivered issue comments to session"
                );
            }
            Err(error) => {
                // Classify this attempt now, from its own execution row: an
                // error response does not mean nothing started. Consent to
                // resend covered only earlier attempts, so an uncertain one
                // here always stops automatic retries.
                for (row, comment) in &ready {
                    match self.latest_attempt(session.id, row.comment_id).await? {
                        EarlierAttempt::Launched(execution_id) => {
                            IssueCommentDelivery::mark_delivered(
                                &self.pool,
                                row.id,
                                execution_id,
                                &comment.message,
                                Some(comment.updated_at),
                                Utc::now(),
                            )
                            .await?;
                        }
                        EarlierAttempt::Uncertain(execution_id) => {
                            self.fail_uncertain(row, execution_id).await?;
                        }
                        EarlierAttempt::None => {
                            self.retry_or_fail(
                                row,
                                IssueCommentDeliveryReason::StartFailed,
                                &format!(
                                    "could not start a turn in session {}: {error}",
                                    session.id
                                ),
                            )
                            .await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Schedule an automatic retry, or fail visibly once the budget is spent.
    async fn retry_or_fail(
        &self,
        row: &IssueCommentDelivery,
        reason: IssueCommentDeliveryReason,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        // `claim` already counted this attempt.
        let attempt = row.attempt_count + 1;
        tracing::warn!(comment_id = %row.comment_id, attempt, error, "issue comment delivery attempt failed");
        match retry_delay(attempt) {
            Some(delay) => {
                IssueCommentDelivery::finish_dispatch(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::Pending,
                    Some(IssueCommentDeliveryReason::RetryScheduled),
                    Some(error),
                    Some(now + chrono::Duration::from_std(delay).unwrap_or_default()),
                    now,
                )
                .await?;
            }
            None => {
                IssueCommentDelivery::finish_dispatch(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::Failed,
                    Some(reason),
                    Some(error),
                    None,
                    now,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Startup: a `dispatching` row means the previous process stopped
    /// between claim and outcome. Resolve it from evidence: a turn carrying
    /// the marker means delivered, otherwise it is retried.
    pub async fn recover(&self) -> Result<(), sqlx::Error> {
        for row in IssueCommentDelivery::list_dispatching(&self.pool).await? {
            let attempt = match row.session_id {
                Some(session_id) => self.earlier_attempt(session_id, row.comment_id).await?,
                None => EarlierAttempt::None,
            };
            let now = Utc::now();
            if let EarlierAttempt::Uncertain(execution_id) = attempt {
                self.fail_uncertain(&row, execution_id).await?;
            } else if let EarlierAttempt::Launched(execution_id) = attempt {
                // The snapshot was recorded before that turn could start.
                IssueCommentDelivery::mark_delivered(
                    &self.pool,
                    row.id,
                    execution_id,
                    row.delivered_message.as_deref().unwrap_or_default(),
                    row.delivered_comment_updated_at,
                    now,
                )
                .await?;
            } else {
                IssueCommentDelivery::finish_dispatch(
                    &self.pool,
                    row.id,
                    IssueCommentDeliveryState::Pending,
                    Some(IssueCommentDeliveryReason::RetryScheduled),
                    Some("delivery was interrupted by a server restart"),
                    None,
                    now,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Periodic sweep: dispatch every session with due pending deliveries.
    pub async fn sweep(&self) -> Result<(), sqlx::Error> {
        crate::wakeups::reconcile(&self.pool).await?;
        for session_id in
            IssueCommentDelivery::sessions_with_due_pending(&self.pool, Utc::now()).await?
        {
            self.dispatch_session(session_id).await?;
        }
        Ok(())
    }

    /// Startup reconciliation is done: orphaned turns were marked and the
    /// resume decision made (resumed turns are running, so they are busy).
    pub fn mark_startup_reconciled(&self) {
        *self
            .reconciled_at
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Utc::now();
    }

    /// A turn boundary was announced for `session_id` (`finalize_task`
    /// recorded it before emitting the event).
    pub async fn on_turn_finalized(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        self.dispatch_session(session_id).await
    }

    /// Comments on an issue with their delivery state (both surfaces).
    pub async fn issue_comments(
        &self,
        issue_id: Uuid,
    ) -> Result<IssueCommentsWithDelivery, DeliveryError> {
        let comments = self.backend.list_comments(issue_id).await?;
        let deliveries = IssueCommentDelivery::list_by_issue(&self.pool, issue_id).await?;
        let project_id = self
            .backend
            .issue_info(issue_id)
            .await
            .ok()
            .map(|issue| issue.project_id);
        let names = match project_id {
            Some(project_id) => self.backend.member_names(project_id).await,
            None => HashMap::new(),
        };
        let (comments, deleted_deliveries) = join_comments(comments, deliveries, &names);
        Ok(IssueCommentsWithDelivery {
            issue_id,
            issue_url: project_id.map(|project_id| issue_url(project_id, issue_id)),
            comments,
            deleted_deliveries,
        })
    }

    /// The linked issue's comments, for a workspace conversation.
    pub async fn workspace_issue_comments(
        &self,
        workspace_id: Uuid,
    ) -> Result<WorkspaceIssueComments, DeliveryError> {
        let Some(issue_id) = self
            .backend
            .workspace_link(workspace_id)
            .await?
            .and_then(|link| link.issue_id)
        else {
            return Ok(WorkspaceIssueComments {
                workspace_id,
                issue: None,
                comments: Vec::new(),
                deleted_deliveries: Vec::new(),
            });
        };
        let issue = self.backend.issue_info(issue_id).await?;
        let comments = self.backend.list_comments(issue_id).await?;
        let deliveries = IssueCommentDelivery::list_by_issue(&self.pool, issue_id).await?;
        let names = self.backend.member_names(issue.project_id).await;
        let (comments, deleted_deliveries) = join_comments(comments, deliveries, &names);
        Ok(WorkspaceIssueComments {
            workspace_id,
            issue: Some(LinkedIssueSummary {
                id: issue.id,
                issue_url: issue_url(issue.project_id, issue.id),
                simple_id: issue.simple_id,
                title: issue.title,
                project_id: issue.project_id,
            }),
            comments,
            deleted_deliveries,
        })
    }

    pub async fn issue_info(&self, issue_id: Uuid) -> Result<IssueInfo, DeliveryError> {
        Ok(self.backend.issue_info(issue_id).await?)
    }

    pub async fn deliveries_for_issue(
        &self,
        issue_id: Uuid,
    ) -> Result<Vec<IssueCommentDelivery>, DeliveryError> {
        Ok(IssueCommentDelivery::list_by_issue(&self.pool, issue_id).await?)
    }
}

/// Browser route of an issue, matching the MCP's `issue_url` (shared by the
/// local and remote apps; never the API host).
pub fn issue_url(project_id: Uuid, issue_id: Uuid) -> String {
    format!("/projects/{project_id}/issues/{issue_id}")
}

fn to_session_candidates(candidates: &[DeliveryTargetCandidate]) -> Vec<SessionCandidate> {
    candidates
        .iter()
        .map(|candidate| SessionCandidate {
            workspace_id: candidate.workspace_id,
            session_id: candidate.session_id,
            agent_state: candidate.agent_state,
        })
        .collect()
}

fn select_target_from(
    candidates: &[DeliveryTargetCandidate],
    explicit_session_id: Option<Uuid>,
    origin_session_id: Option<Uuid>,
) -> TargetDecision {
    services::services::issue_comment_delivery::select_target(
        &to_session_candidates(candidates),
        explicit_session_id,
        origin_session_id,
    )
}

/// Pair comments with their deliveries. Deliveries whose comment is gone are
/// returned separately so a deleted, already-delivered comment stays
/// discoverable.
fn join_comments(
    comments: Vec<IssueComment>,
    deliveries: Vec<IssueCommentDelivery>,
    names: &HashMap<Uuid, String>,
) -> (Vec<IssueCommentWithDelivery>, Vec<IssueCommentDelivery>) {
    let mut by_comment: HashMap<Uuid, IssueCommentDelivery> = deliveries
        .into_iter()
        .map(|delivery| (delivery.comment_id, delivery))
        .collect();
    let present: HashSet<Uuid> = comments.iter().map(|comment| comment.id).collect();
    let joined = comments
        .into_iter()
        .map(|comment| {
            let delivery = by_comment.remove(&comment.id);
            let edited_after_delivery = delivery.as_ref().is_some_and(|delivery| {
                delivery.state == IssueCommentDeliveryState::Delivered
                    && delivery
                        .delivered_comment_updated_at
                        .is_some_and(|delivered| comment.updated_at > delivered)
            });
            IssueCommentWithDelivery {
                author_name: comment.author_id.and_then(|id| names.get(&id).cloned()),
                comment,
                delivery,
                edited_after_delivery,
            }
        })
        .collect();
    let mut deleted: Vec<IssueCommentDelivery> = by_comment
        .into_values()
        .filter(|delivery| !present.contains(&delivery.comment_id))
        .collect();
    deleted.sort_by_key(|delivery| delivery.comment_created_at);
    (joined, deleted)
}

// ── Lifecycle ───────────────────────────────────────────────────────────

pub type Dispatcher = IssueCommentDispatcher<DeploymentBackend>;

static DISPATCHER: std::sync::OnceLock<Arc<Dispatcher>> = std::sync::OnceLock::new();

/// The process-wide dispatcher (created on first use; the background tasks
/// are started by [`spawn`]).
pub fn dispatcher(deployment: &DeploymentImpl) -> Arc<Dispatcher> {
    DISPATCHER
        .get_or_init(|| {
            Arc::new(IssueCommentDispatcher::new(
                deployment.db().pool.clone(),
                DeploymentBackend {
                    deployment: deployment.clone(),
                },
            ))
        })
        .clone()
}

/// Subscribe to turn boundaries, recover interrupted deliveries and run the
/// sweep. Call before boot resumes interrupted agents, since a broadcast sent
/// with no receiver is lost (the sweep would still catch up).
pub fn spawn(deployment: &DeploymentImpl) {
    let dispatcher = dispatcher(deployment);
    if let Some(sender) = deployment.container().turn_finalized_sender() {
        let mut events = sender.subscribe();
        let dispatcher = dispatcher.clone();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => {
                        let dispatcher = dispatcher.clone();
                        // Off the receive loop: a slow remote read for one
                        // session must not delay another session's boundary.
                        tokio::spawn(async move {
                            if let Err(error) = dispatcher.on_turn_finalized(event.session_id).await
                            {
                                tracing::error!(?error, session_id = %event.session_id, "issue comment dispatch failed");
                            }
                        });
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "issue comment dispatcher lagged; the sweep will catch up"
                        );
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        });
    }

    tokio::spawn(async move {
        if let Err(error) = dispatcher.recover().await {
            tracing::error!(?error, "issue comment delivery recovery failed");
        }
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(error) = dispatcher.sweep().await {
                tracing::error!(?error, "issue comment delivery sweep failed");
            }
        }
    });
}

#[cfg(test)]
mod tests;
