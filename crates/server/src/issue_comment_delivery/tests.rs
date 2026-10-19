//! Dispatcher tests: real SQLite (sessions, executions, deliveries) and a
//! fake remote/start backend whose `start_follow_up` inserts a real execution
//! row carrying the prompt, so marker evidence behaves as in production.

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex as StdMutex,
};

use api_types::{IssueComment, ListIssueWorkspacesResponse, Workspace as RemoteWorkspace};
use chrono::{Duration as ChronoDuration, Utc};
use db::models::{
    execution_process::{
        CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
    },
    issue_comment_delivery::{
        IssueCommentDelivery, IssueCommentDeliveryReason, IssueCommentDeliverySource,
        IssueCommentDeliveryState,
    },
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace},
};
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
    },
    executors::BaseCodingAgent,
    profile::ExecutorConfig,
};
use futures_util::future::BoxFuture;
use services::services::{
    issue_comment_delivery::extract_markers, remote_client::RemoteClientError,
};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::*;

#[derive(Default)]
struct FakeState {
    comments: HashMap<Uuid, IssueComment>,
    links: HashMap<Uuid, ListIssueWorkspacesResponse>,
    issue_active: bool,
    issue_error: Option<u16>,
    comment_error: Option<u16>,
    queued: HashSet<Uuid>,
    /// Pending failures for the next starts: `Some(true)` = the start
    /// succeeds but its response is lost; `Some(false)` = it fails outright.
    start_failures: Vec<bool>,
    /// Prompts of every turn actually started.
    starts: Vec<(Uuid, String)>,
}

struct FakeBackend {
    pool: SqlitePool,
    state: StdMutex<FakeState>,
}

fn http(status: u16) -> RemoteClientError {
    RemoteClientError::Http {
        status,
        body: format!("fake {status}"),
    }
}

impl FakeBackend {
    fn state(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.state.lock().unwrap()
    }
}

impl DeliveryBackend for FakeBackend {
    fn get_comment(
        &self,
        comment_id: Uuid,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        Box::pin(async move {
            let state = self.state();
            if let Some(status) = state.comment_error {
                return Err(http(status));
            }
            state
                .comments
                .get(&comment_id)
                .cloned()
                .ok_or_else(|| http(404))
        })
    }

    fn list_comments(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<Vec<IssueComment>, RemoteClientError>> {
        Box::pin(async move {
            let mut comments: Vec<IssueComment> = self
                .state()
                .comments
                .values()
                .filter(|c| c.issue_id == issue_id)
                .cloned()
                .collect();
            comments.sort_by_key(|c| c.created_at);
            Ok(comments)
        })
    }

    fn create_comment(
        &self,
        request: CreateIssueCommentRequest,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        Box::pin(async move {
            let mut state = self.state();
            let id = request.id.unwrap_or_else(Uuid::new_v4);
            if state.comments.contains_key(&id) {
                return Err(http(409));
            }
            let comment = make_comment(id, request.issue_id, &request.message);
            state.comments.insert(id, comment.clone());
            Ok(comment)
        })
    }

    fn issue_workspaces(
        &self,
        issue_id: Uuid,
    ) -> BoxFuture<'_, Result<ListIssueWorkspacesResponse, RemoteClientError>> {
        Box::pin(async move {
            let state = self.state();
            if let Some(status) = state.issue_error {
                return Err(http(status));
            }
            Ok(state
                .links
                .get(&issue_id)
                .cloned()
                .unwrap_or(ListIssueWorkspacesResponse {
                    workspaces: Vec::new(),
                    archived_count: 0,
                }))
        })
    }

    fn workspace_link(
        &self,
        local_workspace_id: Uuid,
    ) -> BoxFuture<'_, Result<Option<RemoteWorkspace>, RemoteClientError>> {
        Box::pin(async move {
            Ok(self
                .state()
                .links
                .values()
                .flat_map(|links| links.workspaces.iter())
                .find(|w| w.local_workspace_id == Some(local_workspace_id))
                .cloned())
        })
    }

    fn issue_info(&self, issue_id: Uuid) -> BoxFuture<'_, Result<IssueInfo, RemoteClientError>> {
        Box::pin(async move {
            let state = self.state();
            if let Some(status) = state.issue_error {
                return Err(http(status));
            }
            Ok(IssueInfo {
                id: issue_id,
                simple_id: "VAS-859".into(),
                title: "Rust AMT MCP".into(),
                project_id: Uuid::nil(),
                active: state.issue_active,
            })
        })
    }

    fn member_names(&self, _project_id: Uuid) -> BoxFuture<'_, HashMap<Uuid, String>> {
        Box::pin(async { HashMap::new() })
    }

    fn has_queued_message(&self, session_id: Uuid) -> bool {
        self.state().queued.contains(&session_id)
    }

    fn start_follow_up(
        &self,
        session_id: Uuid,
        prompt: String,
    ) -> BoxFuture<'_, Result<Uuid, String>> {
        Box::pin(async move {
            let failure = {
                let mut state = self.state();
                if state.start_failures.is_empty() {
                    None
                } else {
                    Some(state.start_failures.remove(0))
                }
            };
            if failure == Some(false) {
                return Err("container unavailable".into());
            }
            let id = insert_turn(
                &self.pool,
                session_id,
                &prompt,
                ExecutionProcessStatus::Running,
                None,
            )
            .await;
            self.state().starts.push((session_id, prompt));
            if failure == Some(true) {
                return Err("response lost".into());
            }
            Ok(id)
        })
    }
}

fn make_comment(id: Uuid, issue_id: Uuid, message: &str) -> IssueComment {
    let now = Utc::now();
    IssueComment {
        id,
        issue_id,
        author_id: Some(Uuid::new_v4()),
        parent_id: None,
        message: message.to_string(),
        created_at: now,
        updated_at: now,
    }
}

async fn insert_turn(
    pool: &SqlitePool,
    session_id: Uuid,
    prompt: &str,
    status: ExecutionProcessStatus,
    completed_ago: Option<i64>,
) -> Uuid {
    let id = Uuid::new_v4();
    let action = ExecutorAction::new(
        ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
            prompt: prompt.to_string(),
            session_id: "agent-session".into(),
            reset_to_message_id: None,
            executor_config: ExecutorConfig::new(BaseCodingAgent::ClaudeCode),
            working_dir: None,
        }),
        None,
    );
    ExecutionProcess::create(
        pool,
        &CreateExecutionProcess {
            session_id,
            executor_action: action,
            run_reason: ExecutionProcessRunReason::CodingAgent,
        },
        id,
        &[],
    )
    .await
    .unwrap();
    if status != ExecutionProcessStatus::Running {
        ExecutionProcess::update_completion(pool, id, status, Some(0))
            .await
            .unwrap();
        if let Some(seconds) = completed_ago {
            sqlx::query("UPDATE execution_processes SET completed_at = ? WHERE id = ?")
                .bind(Utc::now() - ChronoDuration::seconds(seconds))
                .bind(id)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    id
}

struct Fixture {
    pool: SqlitePool,
    dispatcher: IssueCommentDispatcher<std::sync::Arc<FakeBackend>>,
    backend: std::sync::Arc<FakeBackend>,
    issue_id: Uuid,
}

impl DeliveryBackend for std::sync::Arc<FakeBackend> {
    fn get_comment(&self, id: Uuid) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        (**self).get_comment(id)
    }
    fn list_comments(
        &self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<Vec<IssueComment>, RemoteClientError>> {
        (**self).list_comments(id)
    }
    fn create_comment(
        &self,
        request: CreateIssueCommentRequest,
    ) -> BoxFuture<'_, Result<IssueComment, RemoteClientError>> {
        (**self).create_comment(request)
    }
    fn issue_workspaces(
        &self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<ListIssueWorkspacesResponse, RemoteClientError>> {
        (**self).issue_workspaces(id)
    }
    fn workspace_link(
        &self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<Option<RemoteWorkspace>, RemoteClientError>> {
        (**self).workspace_link(id)
    }
    fn issue_info(&self, id: Uuid) -> BoxFuture<'_, Result<IssueInfo, RemoteClientError>> {
        (**self).issue_info(id)
    }
    fn member_names(&self, id: Uuid) -> BoxFuture<'_, HashMap<Uuid, String>> {
        (**self).member_names(id)
    }
    fn has_queued_message(&self, id: Uuid) -> bool {
        (**self).has_queued_message(id)
    }
    fn start_follow_up(&self, id: Uuid, prompt: String) -> BoxFuture<'_, Result<Uuid, String>> {
        (**self).start_follow_up(id, prompt)
    }
}

impl Fixture {
    async fn new() -> Self {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        let backend = std::sync::Arc::new(FakeBackend {
            pool: pool.clone(),
            state: StdMutex::new(FakeState {
                issue_active: true,
                ..Default::default()
            }),
        });
        Self {
            dispatcher: IssueCommentDispatcher::new(pool.clone(), backend.clone()),
            pool,
            backend,
            issue_id: Uuid::new_v4(),
        }
    }

    /// A local workspace linked to the issue, with one session.
    async fn linked_workspace(&self) -> (Uuid, Uuid) {
        let workspace_id = Uuid::new_v4();
        Workspace::create(
            &self.pool,
            &CreateWorkspace {
                branch: format!("vk/{workspace_id}"),
                name: None,
            },
            workspace_id,
        )
        .await
        .unwrap();
        let session_id = self.add_session(workspace_id).await;
        let mut state = self.backend.state();
        let links = state
            .links
            .entry(self.issue_id)
            .or_insert(ListIssueWorkspacesResponse {
                workspaces: Vec::new(),
                archived_count: 0,
            });
        links.workspaces.push(RemoteWorkspace {
            id: Uuid::new_v4(),
            project_id: Uuid::nil(),
            owner_user_id: Uuid::nil(),
            issue_id: Some(self.issue_id),
            local_workspace_id: Some(workspace_id),
            name: None,
            archived: false,
            files_changed: None,
            lines_added: None,
            lines_removed: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
        (workspace_id, session_id)
    }

    async fn add_session(&self, workspace_id: Uuid) -> Uuid {
        let session_id = Uuid::new_v4();
        Session::create(
            &self.pool,
            &CreateSession {
                executor: Some("CLAUDE_CODE".into()),
                name: None,
            },
            session_id,
            workspace_id,
        )
        .await
        .unwrap();
        session_id
    }

    /// The agent asked a question and its turn completed a while ago.
    async fn waiting(&self, session_id: Uuid) -> Uuid {
        insert_turn(
            &self.pool,
            session_id,
            "Implement the AMT MCP",
            ExecutionProcessStatus::Completed,
            Some(600),
        )
        .await
    }

    fn post(&self, message: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.backend
            .state()
            .comments
            .insert(id, make_comment(id, self.issue_id, message));
        id
    }

    async fn reply(&self, comment_id: Uuid, target: Option<Uuid>) -> IssueCommentDelivery {
        self.dispatcher
            .request(RequestIssueCommentDelivery {
                comment_id,
                issue_id: self.issue_id,
                target_session_id: target,
                source: None,
                origin_session_id: None,
                origin_workspace_id: None,
            })
            .await
            .unwrap()
    }

    fn starts(&self) -> Vec<(Uuid, String)> {
        self.backend.state().starts.clone()
    }

    async fn complete_running(&self, session_id: Uuid, completed_ago: i64) {
        sqlx::query(
            "UPDATE execution_processes SET status = 'completed', completed_at = ? \
             WHERE session_id = ? AND status = 'running'",
        )
        .bind(Utc::now() - ChronoDuration::seconds(completed_ago))
        .bind(session_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

// AC1/AC2: a waiting agent is resumed once by a single reply.
#[tokio::test]
async fn reply_resumes_the_single_waiting_session_once() {
    let fx = Fixture::new().await;
    let (workspace_id, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    let comment = fx.post("Device address is 10.0.0.5");

    let delivery = fx.reply(comment, None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(delivery.session_id, Some(session_id));
    assert_eq!(delivery.workspace_id, Some(workspace_id));
    assert_eq!(
        delivery.delivered_message.as_deref(),
        Some("Device address is 10.0.0.5")
    );
    let starts = fx.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(extract_markers(&starts[0].1), vec![comment]);
    assert!(starts[0].1.contains("> Device address is 10.0.0.5"));

    // Duplicate client request: same row, no second turn.
    let again = fx.reply(comment, None).await;
    assert_eq!(again.id, delivery.id);
    assert_eq!(again.execution_process_id, delivery.execution_process_id);
    fx.dispatcher.sweep().await.unwrap();
    assert_eq!(fx.starts().len(), 1);
}

// AC4, ordering 1: the reply arrives while the agent runs, then the turn ends.
#[tokio::test]
async fn running_agent_gets_one_turn_after_its_boundary() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Running,
        None,
    )
    .await;
    let first = fx.post("first clarification");
    let second = fx.post("second clarification");

    let a = fx.reply(first, None).await;
    let b = fx.reply(second, None).await;
    for delivery in [&a, &b] {
        assert_eq!(delivery.state, IssueCommentDeliveryState::Pending);
        assert_eq!(
            delivery.reason,
            Some(IssueCommentDeliveryReason::AgentRunning)
        );
    }
    fx.dispatcher.sweep().await.unwrap();
    assert!(
        fx.starts().is_empty(),
        "no competing execution while running"
    );

    fx.complete_running(session_id, 0).await;
    fx.dispatcher.on_turn_finalized(session_id).await.unwrap();
    let starts = fx.starts();
    assert_eq!(starts.len(), 1, "both comments ride one turn");
    assert_eq!(extract_markers(&starts[0].1), vec![first, second]);
    let a = IssueCommentDelivery::find_by_id(&fx.pool, a.id)
        .await
        .unwrap()
        .unwrap();
    let b = IssueCommentDelivery::find_by_id(&fx.pool, b.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(a.execution_process_id, b.execution_process_id);
}

// Ordering 2: the turn just completed and no boundary event has been seen
// yet (the exit monitor may still hand off its queued message). The reply
// waits; the boundary event then delivers it.
#[tokio::test]
async fn reply_right_after_completion_waits_for_the_boundary_event() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Completed,
        None,
    )
    .await;
    let comment = fx.post("answer");

    let delivery = fx.reply(comment, None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::Pending);
    fx.dispatcher.sweep().await.unwrap();
    assert!(fx.starts().is_empty());

    fx.dispatcher.on_turn_finalized(session_id).await.unwrap();
    assert_eq!(fx.starts().len(), 1);
}

// Ordering 3: the boundary event was already seen, so a reply that arrives
// just after it is delivered at once.
#[tokio::test]
async fn reply_after_a_seen_boundary_is_delivered_immediately() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Completed,
        None,
    )
    .await;
    fx.dispatcher.on_turn_finalized(session_id).await.unwrap();

    let delivery = fx.reply(fx.post("answer"), None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(fx.starts().len(), 1);
}

#[tokio::test]
async fn a_queued_follow_up_goes_first() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    fx.backend.state().queued.insert(session_id);

    let delivery = fx.reply(fx.post("answer"), None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::Pending);
    assert_eq!(
        delivery.reason,
        Some(IssueCommentDeliveryReason::QueuedMessage)
    );
    assert!(fx.starts().is_empty());
}

#[tokio::test]
async fn ambiguous_targets_need_an_explicit_choice() {
    let fx = Fixture::new().await;
    let (_, a) = fx.linked_workspace().await;
    let (_, b) = fx.linked_workspace().await;
    fx.waiting(a).await;
    fx.waiting(b).await;
    let comment = fx.post("which one?");

    let targets = fx.dispatcher.agent_targets(fx.issue_id).await.unwrap();
    assert_eq!(targets.candidates.len(), 2);
    assert_eq!(targets.default_session_id, None);

    let delivery = fx.reply(comment, None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::NeedsTarget);
    assert_eq!(delivery.reason, Some(IssueCommentDeliveryReason::Ambiguous));
    assert!(fx.starts().is_empty(), "never broadcast");

    let chosen = fx.reply(comment, Some(b)).await;
    assert_eq!(chosen.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(chosen.session_id, Some(b));
    assert_eq!(fx.starts().len(), 1);
    assert_eq!(fx.starts()[0].0, b);
}

#[tokio::test]
async fn no_linked_workspace_is_reported_not_delivered() {
    let fx = Fixture::new().await;
    let delivery = fx.reply(fx.post("hello?"), None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::NoTarget);
    assert_eq!(
        delivery.reason,
        Some(IssueCommentDeliveryReason::NoLinkedWorkspace)
    );
}

#[tokio::test]
async fn workspace_on_another_host_is_not_a_target() {
    let fx = Fixture::new().await;
    let (workspace_id, _) = fx.linked_workspace().await;
    Workspace::delete(&fx.pool, workspace_id).await.unwrap();
    let delivery = fx.reply(fx.post("hello?"), None).await;
    assert_eq!(
        delivery.reason,
        Some(IssueCommentDeliveryReason::WorkspaceOtherHost)
    );
}

// AC3: archived workspaces and Done/Cancelled issues never auto-start.
#[tokio::test]
async fn archived_workspace_and_terminal_issue_never_start() {
    let fx = Fixture::new().await;
    let (workspace_id, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    Workspace::set_archived(&fx.pool, workspace_id, true)
        .await
        .unwrap();
    let archived = fx.reply(fx.post("note"), Some(session_id)).await;
    assert_eq!(archived.state, IssueCommentDeliveryState::NoTarget);
    assert_eq!(
        archived.reason,
        Some(IssueCommentDeliveryReason::WorkspaceArchived)
    );

    Workspace::set_archived(&fx.pool, workspace_id, false)
        .await
        .unwrap();
    fx.backend.state().issue_active = false;
    let done = fx.reply(fx.post("thanks!"), Some(session_id)).await;
    assert_eq!(done.state, IssueCommentDeliveryState::NoTarget);
    assert_eq!(done.reason, Some(IssueCommentDeliveryReason::IssueTerminal));
    assert!(fx.starts().is_empty());
}

#[tokio::test]
async fn stopped_turn_resumes_only_by_explicit_choice() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Killed,
        Some(600),
    )
    .await;
    let comment = fx.post("try again with this");

    let implicit = fx.reply(comment, None).await;
    assert_eq!(implicit.state, IssueCommentDeliveryState::NoTarget);
    assert_eq!(
        implicit.reason,
        Some(IssueCommentDeliveryReason::TurnStopped)
    );
    assert!(fx.starts().is_empty());

    let explicit = fx.reply(comment, Some(session_id)).await;
    assert_eq!(explicit.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(fx.starts().len(), 1);
}

#[tokio::test]
async fn a_turn_that_fails_while_a_reply_waits_is_not_resumed() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Running,
        None,
    )
    .await;
    let delivery = fx.reply(fx.post("answer"), None).await;
    assert_eq!(delivery.state, IssueCommentDeliveryState::Pending);

    sqlx::query(
        "UPDATE execution_processes SET status = 'failed', completed_at = ? WHERE session_id = ?",
    )
    .bind(Utc::now())
    .bind(session_id)
    .execute(&fx.pool)
    .await
    .unwrap();
    fx.dispatcher.on_turn_finalized(session_id).await.unwrap();
    let stored = IssueCommentDelivery::find_by_id(&fx.pool, delivery.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, IssueCommentDeliveryState::NoTarget);
    assert_eq!(stored.reason, Some(IssueCommentDeliveryReason::TurnStopped));
    assert!(fx.starts().is_empty());
}

// Authorization: a user who cannot read the comment creates nothing.
#[tokio::test]
async fn authorization_failure_creates_no_delivery() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    let comment = fx.post("secret");
    fx.backend.state().comment_error = Some(403);

    let error = fx
        .dispatcher
        .request(RequestIssueCommentDelivery {
            comment_id: comment,
            issue_id: fx.issue_id,
            target_session_id: None,
            source: None,
            origin_session_id: None,
            origin_workspace_id: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(error, DeliveryError::Remote(ref e) if is_status(e, 403)));
    assert!(
        IssueCommentDelivery::find_by_comment_id(&fx.pool, comment)
            .await
            .unwrap()
            .is_none()
    );
    assert!(fx.starts().is_empty());
}

#[tokio::test]
async fn remote_outage_is_a_retryable_failure_not_no_target() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    let comment = fx.post("answer");
    fx.backend.state().issue_error = Some(503);

    let failed = fx.reply(comment, None).await;
    assert_eq!(failed.state, IssueCommentDeliveryState::Failed);
    assert_eq!(
        failed.reason,
        Some(IssueCommentDeliveryReason::RemoteUnavailable)
    );
    assert!(failed.error.as_deref().unwrap().contains("503"));

    fx.backend.state().issue_error = None;
    let retried = fx.reply(comment, None).await;
    assert_eq!(retried.state, IssueCommentDeliveryState::Delivered);
}

// Backend delivery failure: bounded automatic retries, then a visible
// failure; a manual retry starts over.
#[tokio::test]
async fn start_failures_retry_then_fail_visibly() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    fx.backend.state().start_failures = vec![false, false, false];
    let comment = fx.post("answer");

    let first = fx.reply(comment, None).await;
    assert_eq!(first.state, IssueCommentDeliveryState::Pending);
    assert_eq!(
        first.reason,
        Some(IssueCommentDeliveryReason::RetryScheduled)
    );
    assert!(first.next_attempt_at.is_some());
    assert!(
        first
            .error
            .as_deref()
            .unwrap()
            .contains("container unavailable")
    );

    for _ in 0..2 {
        sqlx::query("UPDATE issue_comment_deliveries SET next_attempt_at = ? WHERE id = ?")
            .bind(Utc::now() - ChronoDuration::seconds(1))
            .bind(first.id)
            .execute(&fx.pool)
            .await
            .unwrap();
        fx.dispatcher.sweep().await.unwrap();
    }
    let failed = IssueCommentDelivery::find_by_id(&fx.pool, first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, IssueCommentDeliveryState::Failed);
    assert_eq!(failed.reason, Some(IssueCommentDeliveryReason::StartFailed));
    assert_eq!(failed.attempt_count, 3);
    assert!(fx.starts().is_empty());

    let manual = fx.reply(comment, None).await;
    assert_eq!(manual.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(fx.starts().len(), 1);
}

// A start whose response was lost is recognised from the marker on retry;
// no second turn is started.
#[tokio::test]
async fn lost_start_response_is_recovered_from_evidence() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    fx.backend.state().start_failures = vec![true];
    let comment = fx.post("answer");

    let first = fx.reply(comment, None).await;
    assert_eq!(first.state, IssueCommentDeliveryState::Pending);
    assert_eq!(fx.starts().len(), 1);

    // The started turn finishes; the retry comes due.
    fx.complete_running(session_id, 60).await;
    sqlx::query("UPDATE issue_comment_deliveries SET next_attempt_at = ? WHERE id = ?")
        .bind(Utc::now() - ChronoDuration::seconds(1))
        .bind(first.id)
        .execute(&fx.pool)
        .await
        .unwrap();
    fx.dispatcher.sweep().await.unwrap();

    let stored = IssueCommentDelivery::find_by_id(&fx.pool, first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(fx.starts().len(), 1, "no duplicate execution");
}

// Restart recovery: a row left `dispatching` by a dead process.
#[tokio::test]
async fn restart_recovery_uses_positive_evidence() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    fx.backend.state().queued.insert(session_id);
    let started = fx.post("started before the crash");
    let not_started = fx.post("claimed but never started");
    let a = fx.reply(started, None).await;
    let b = fx.reply(not_started, None).await;
    fx.backend.state().queued.clear();
    for row in [&a, &b] {
        assert!(
            IssueCommentDelivery::claim(&fx.pool, row.id, Utc::now())
                .await
                .unwrap()
        );
    }
    // Only the first comment's turn exists.
    let prompt = compose_delivery_prompt(
        "VAS-859",
        &[DeliveredComment {
            comment_id: started,
            author_name: None,
            created_at: Utc::now(),
            message: "started before the crash".into(),
        }],
    );
    let execution = insert_turn(
        &fx.pool,
        session_id,
        &prompt,
        ExecutionProcessStatus::Completed,
        Some(60),
    )
    .await;

    fx.dispatcher.recover().await.unwrap();
    let a = IssueCommentDelivery::find_by_id(&fx.pool, a.id)
        .await
        .unwrap()
        .unwrap();
    let b = IssueCommentDelivery::find_by_id(&fx.pool, b.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a.state, IssueCommentDeliveryState::Delivered);
    assert_eq!(a.execution_process_id, Some(execution));
    assert_eq!(b.state, IssueCommentDeliveryState::Pending);

    fx.dispatcher.sweep().await.unwrap();
    let starts = fx.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(extract_markers(&starts[0].1), vec![not_started]);
}

// Edits: a pending delivery sends the latest text; a delivered one is a
// snapshot and is flagged when edited later. Deletion before dispatch cancels.
#[tokio::test]
async fn edits_and_deletions_follow_snapshot_semantics() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_id,
        "work",
        ExecutionProcessStatus::Running,
        None,
    )
    .await;
    let edited = fx.post("port 16992");
    let deleted = fx.post("ignore this");
    let a = fx.reply(edited, None).await;
    let b = fx.reply(deleted, None).await;

    {
        let mut state = fx.backend.state();
        let comment = state.comments.get_mut(&edited).unwrap();
        comment.message = "port 16993".into();
        comment.updated_at = Utc::now();
        state.comments.remove(&deleted);
    }
    fx.complete_running(session_id, 0).await;
    fx.dispatcher.on_turn_finalized(session_id).await.unwrap();

    let starts = fx.starts();
    assert_eq!(starts.len(), 1);
    assert!(starts[0].1.contains("> port 16993"));
    assert!(!starts[0].1.contains("ignore this"));
    let b = IssueCommentDelivery::find_by_id(&fx.pool, b.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b.state, IssueCommentDeliveryState::Cancelled);
    assert_eq!(b.reason, Some(IssueCommentDeliveryReason::CommentDeleted));

    // Edited after delivery: flagged, never replayed.
    {
        let mut state = fx.backend.state();
        let comment = state.comments.get_mut(&edited).unwrap();
        comment.message = "port 443".into();
        comment.updated_at = Utc::now() + ChronoDuration::seconds(1);
    }
    let again = fx.reply(edited, None).await;
    assert_eq!(again.id, a.id);
    assert_eq!(again.delivered_message.as_deref(), Some("port 16993"));
    let listed = fx.dispatcher.issue_comments(fx.issue_id).await.unwrap();
    let entry = listed
        .comments
        .iter()
        .find(|c| c.comment.id == edited)
        .unwrap();
    assert!(entry.edited_after_delivery);
    assert_eq!(fx.starts().len(), 1);
}

#[tokio::test]
async fn agents_cannot_resume_themselves_or_loop_forever() {
    let fx = Fixture::new().await;
    let (workspace_a, session_a) = fx.linked_workspace().await;
    insert_turn(
        &fx.pool,
        session_a,
        "work",
        ExecutionProcessStatus::Running,
        None,
    )
    .await;

    // An agent's reply on its own issue does not target its own session.
    let own = fx
        .dispatcher
        .create_comment(CreateIssueCommentWithIntent {
            issue_id: fx.issue_id,
            message: "status update".into(),
            intent: IssueCommentIntent::ReplyToAgent,
            parent_id: None,
            target_session_id: None,
            comment_id: None,
            origin_session_id: None,
            origin_workspace_id: Some(workspace_a),
        })
        .await
        .unwrap();
    let own = own.2.unwrap();
    assert_eq!(own.state, IssueCommentDeliveryState::Blocked);
    assert_eq!(own.reason, Some(IssueCommentDeliveryReason::SelfTarget));

    // A chain of agent-started turns is cut at the hop limit.
    let (_, session_b) = fx.linked_workspace().await;
    fx.waiting(session_b).await;
    let origin_turn = insert_turn(
        &fx.pool,
        session_a,
        "hop",
        ExecutionProcessStatus::Running,
        None,
    )
    .await;
    sqlx::query(
        "UPDATE issue_comment_deliveries SET execution_process_id = ?, hop_count = 2 WHERE id = ?",
    )
    .bind(origin_turn)
    .bind(own.id)
    .execute(&fx.pool)
    .await
    .unwrap();
    let looped = fx
        .dispatcher
        .create_comment(CreateIssueCommentWithIntent {
            issue_id: fx.issue_id,
            message: "ping".into(),
            intent: IssueCommentIntent::ReplyToAgent,
            parent_id: None,
            target_session_id: Some(session_b),
            comment_id: None,
            origin_session_id: Some(session_a),
            origin_workspace_id: None,
        })
        .await
        .unwrap()
        .2
        .unwrap();
    assert_eq!(looped.state, IssueCommentDeliveryState::Blocked);
    assert_eq!(looped.reason, Some(IssueCommentDeliveryReason::LoopGuard));
    assert!(fx.starts().is_empty());
}

#[tokio::test]
async fn notes_never_deliver_and_create_is_idempotent() {
    let fx = Fixture::new().await;
    let (_, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    let comment_id = Uuid::new_v4();
    let note = CreateIssueCommentWithIntent {
        issue_id: fx.issue_id,
        message: "FYI: deployed".into(),
        intent: IssueCommentIntent::Note,
        parent_id: None,
        target_session_id: None,
        comment_id: Some(comment_id),
        origin_session_id: None,
        origin_workspace_id: None,
    };
    let (comment, _, delivery) = fx.dispatcher.create_comment(note.clone()).await.unwrap();
    assert_eq!(comment.id, comment_id);
    assert!(delivery.is_none());
    // A retried create with the same id returns the same comment.
    let (again, _, _) = fx.dispatcher.create_comment(note.clone()).await.unwrap();
    assert_eq!(again.id, comment_id);
    assert_eq!(fx.backend.state().comments.len(), 1);
    // Reusing the id for different text is a conflict, not a silent merge.
    let conflict = fx
        .dispatcher
        .create_comment(CreateIssueCommentWithIntent {
            message: "something else".into(),
            ..note
        })
        .await
        .unwrap_err();
    assert!(matches!(conflict, DeliveryError::Conflict(_)));
    assert!(fx.starts().is_empty());
    assert!(
        IssueCommentDelivery::find_by_comment_id(&fx.pool, comment_id)
            .await
            .unwrap()
            .is_none()
    );
    // MCP replies are attributed to the MCP source.
    let reply = fx
        .dispatcher
        .create_comment(CreateIssueCommentWithIntent {
            issue_id: fx.issue_id,
            message: "address 10.0.0.5".into(),
            intent: IssueCommentIntent::ReplyToAgent,
            parent_id: None,
            target_session_id: None,
            comment_id: None,
            origin_session_id: None,
            origin_workspace_id: None,
        })
        .await
        .unwrap()
        .2
        .unwrap();
    assert_eq!(reply.source, IssueCommentDeliverySource::Mcp);
    assert_eq!(reply.state, IssueCommentDeliveryState::Delivered);
}

#[tokio::test]
async fn workspace_conversation_lists_linked_comments_with_deliveries() {
    let fx = Fixture::new().await;
    let (workspace_id, session_id) = fx.linked_workspace().await;
    fx.waiting(session_id).await;
    let note = fx.post("just a note");
    let reply = fx.post("the answer");
    fx.reply(reply, None).await;

    let listed = fx
        .dispatcher
        .workspace_issue_comments(workspace_id)
        .await
        .unwrap();
    let issue = listed.issue.unwrap();
    assert_eq!(
        issue.issue_url,
        format!("/projects/{}/issues/{}", Uuid::nil(), fx.issue_id)
    );
    let by_id: HashMap<Uuid, &IssueCommentWithDelivery> =
        listed.comments.iter().map(|c| (c.comment.id, c)).collect();
    assert!(by_id[&note].delivery.is_none());
    assert_eq!(
        by_id[&reply].delivery.as_ref().unwrap().state,
        IssueCommentDeliveryState::Delivered
    );

    // A deleted, delivered comment stays discoverable.
    fx.backend.state().comments.remove(&reply);
    let listed = fx
        .dispatcher
        .workspace_issue_comments(workspace_id)
        .await
        .unwrap();
    assert_eq!(listed.deleted_deliveries.len(), 1);

    let unlinked = fx
        .dispatcher
        .workspace_issue_comments(Uuid::new_v4())
        .await
        .unwrap();
    assert!(unlinked.issue.is_none());
}
