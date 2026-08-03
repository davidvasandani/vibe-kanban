use chrono::{DateTime, Utc};
use executors::actions::{ExecutorAction, ExecutorActionType};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type, types::Json};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

/// Maximum length for auto-generated workspace names (derived from first user prompt)
const WORKSPACE_NAME_MAX_LEN: usize = 60;

use super::{
    execution_process::ExecutorActionField,
    session::Session,
    workspace_repo::{RepoWithTargetBranch, WorkspaceRepo},
};

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub workspace_id: Uuid,
}

#[derive(Debug)]
struct WorkspaceContainerRefRow {
    id: Uuid,
    container_ref: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Workspace {
    pub id: Uuid,
    pub task_id: Option<Uuid>,
    pub container_ref: Option<String>,
    pub branch: String,
    pub setup_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived: bool,
    pub pinned: bool,
    pub name: Option<String>,
    pub worktree_deleted: bool,
    /// Which numbered `## Pipeline` stage the execution agent last reported
    /// itself as starting (1-based), detected from a `VK-PIPELINE-STAGE: N`
    /// marker in the execution's raw log stream. `None` when no coding-agent
    /// execution has reported a stage yet for the current run.
    pub current_pipeline_stage: Option<i64>,
    /// SpecKit feature key: the workspace's branch, captured verbatim at first
    /// SpecKit provisioning. Artifacts live under `specs/<feature_key>/` in the
    /// spec-host repo. Once set it is never re-derived.
    pub speckit_feature_key: Option<String>,
    /// Which repo worktree hosts `specs/` + `.specify/` for this workspace's
    /// SpecKit artifacts. Persisted at first provisioning.
    pub speckit_host_repo_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceWithStatus {
    #[serde(flatten)]
    #[ts(flatten)]
    pub workspace: Workspace,
    pub is_running: bool,
    pub is_errored: bool,
}

impl std::ops::Deref for WorkspaceWithStatus {
    type Target = Workspace;
    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateFollowUpAttempt {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub workspace: Workspace,
    pub workspace_repos: Vec<RepoWithTargetBranch>,
    pub orchestrator_session_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateWorkspace {
    pub branch: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, TS)]
#[sqlx(type_name = "workspace_placement_state", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum WorkspacePlacementState {
    Local,
    Reserved,
    Provisioning,
    Ready,
    Failed,
    Cleaning,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct WorkspacePlacement {
    pub workspace_id: Uuid,
    pub worker_node_id: Option<Uuid>,
    pub placement_state: WorkspacePlacementState,
    pub placed_at: Option<DateTime<Utc>>,
    pub placement_reason: Option<String>,
    pub requested_worker_node_id: Option<Uuid>,
    #[ts(type = "unknown")]
    pub placement_constraints: Option<Json<serde_json::Value>>,
}

impl WorkspacePlacement {
    pub async fn find(pool: &SqlitePool, workspace_id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(
            r#"
            SELECT id AS workspace_id, worker_node_id, placement_state,
                   placed_at, placement_reason, requested_worker_node_id,
                   placement_constraints
            FROM workspaces
            WHERE id = ?
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(pool)
        .await
    }

    pub async fn reserve(
        pool: &SqlitePool,
        workspace_id: Uuid,
        worker_node_id: Uuid,
        requested_worker_node_id: Option<Uuid>,
        constraints: Option<&serde_json::Value>,
        reason: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET worker_node_id = ?,
                placement_state = 'reserved',
                placed_at = datetime('now', 'subsec'),
                placement_reason = ?,
                requested_worker_node_id = ?,
                placement_constraints = ?,
                updated_at = datetime('now', 'subsec')
            WHERE id = ? AND placement_state = 'local' AND worker_node_id IS NULL
            "#,
        )
        .bind(worker_node_id)
        .bind(reason)
        .bind(requested_worker_node_id)
        .bind(constraints.map(Json))
        .bind(workspace_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn transition(
        pool: &SqlitePool,
        workspace_id: Uuid,
        expected: WorkspacePlacementState,
        next: WorkspacePlacementState,
        reason: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let allowed = matches!(
            (expected, next),
            (
                WorkspacePlacementState::Reserved,
                WorkspacePlacementState::Provisioning
            ) | (
                WorkspacePlacementState::Provisioning,
                WorkspacePlacementState::Ready
            ) | (
                WorkspacePlacementState::Provisioning,
                WorkspacePlacementState::Failed
            )
        );
        if !allowed {
            return Ok(false);
        }

        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET placement_state = ?,
                placement_reason = COALESCE(?, placement_reason),
                updated_at = datetime('now', 'subsec')
            WHERE id = ? AND placement_state = ?
            "#,
        )
        .bind(next)
        .bind(reason)
        .bind(workspace_id)
        .bind(expected)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn begin_cleanup(
        pool: &SqlitePool,
        workspace_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE workspaces
            SET placement_state = 'cleaning',
                updated_at = datetime('now', 'subsec')
            WHERE id = ?
              AND placement_state = 'ready'
              AND worker_node_id IN (
                SELECT id FROM worker_nodes
                WHERE status = 'online'
                  AND mount_status = 'healthy'
                  AND lease_expires_at > ?
              )
              AND NOT EXISTS (
                SELECT 1
                FROM sessions s
                JOIN execution_processes ep ON ep.session_id = s.id
                JOIN execution_worker_jobs ewj ON ewj.execution_process_id = ep.id
                WHERE s.workspace_id = workspaces.id
                  AND ewj.dispatch_state NOT IN ('completed', 'failed', 'killed')
              )
            "#,
        )
        .bind(workspace_id)
        .bind(now)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

impl Workspace {
    /// Fetch all workspaces. Newest first.
    pub async fn fetch_all(pool: &SqlitePool) -> Result<Vec<Self>, WorkspaceError> {
        let workspaces = sqlx::query_as!(
            Workspace,
            r#"SELECT id AS "id!: Uuid",
                          task_id AS "task_id: Uuid",
                          container_ref,
                          branch,
                          setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                          created_at AS "created_at!: DateTime<Utc>",
                          updated_at AS "updated_at!: DateTime<Utc>",
                          archived AS "archived!: bool",
                          pinned AS "pinned!: bool",
                          name,
                          worktree_deleted AS "worktree_deleted!: bool",
                          current_pipeline_stage,
                          speckit_feature_key,
                          speckit_host_repo_id AS "speckit_host_repo_id: Uuid"
                   FROM workspaces
                   ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
        .map_err(WorkspaceError::Database)?;

        Ok(workspaces)
    }

    /// Load full workspace context by workspace ID.
    pub async fn load_context(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<WorkspaceContext, WorkspaceError> {
        let workspace = Workspace::find_by_id(pool, workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound)?;

        let workspace_repos =
            WorkspaceRepo::find_repos_with_target_branch_for_workspace(pool, workspace_id).await?;
        let orchestrator_session_id = Session::find_first_by_workspace_id(pool, workspace_id)
            .await?
            .map(|session| session.id);

        Ok(WorkspaceContext {
            workspace,
            workspace_repos,
            orchestrator_session_id,
        })
    }

    /// Update container reference
    pub async fn update_container_ref(
        pool: &SqlitePool,
        workspace_id: Uuid,
        container_ref: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        sqlx::query!(
            "UPDATE workspaces SET container_ref = $1, updated_at = $2 WHERE id = $3",
            container_ref,
            now,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_worktree_deleted(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET worktree_deleted = TRUE, updated_at = datetime('now') WHERE id = ?",
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn clear_worktree_deleted(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET worktree_deleted = FALSE, updated_at = datetime('now') WHERE id = ?",
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update the workspace's updated_at timestamp to prevent cleanup.
    /// Call this when the workspace is accessed (e.g., opened in editor).
    pub async fn touch(pool: &SqlitePool, workspace_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET updated_at = datetime('now', 'subsec') WHERE id = ?",
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT  id                AS "id!: Uuid",
                       task_id           AS "task_id: Uuid",
                       container_ref,
                       branch,
                       setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                       created_at        AS "created_at!: DateTime<Utc>",
                       updated_at        AS "updated_at!: DateTime<Utc>",
                       archived          AS "archived!: bool",
                       pinned            AS "pinned!: bool",
                       name,
                       worktree_deleted  AS "worktree_deleted!: bool",
                       current_pipeline_stage,
                       speckit_feature_key,
                       speckit_host_repo_id AS "speckit_host_repo_id: Uuid"
               FROM    workspaces
               WHERE   id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_rowid(pool: &SqlitePool, rowid: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT  id                AS "id!: Uuid",
                       task_id           AS "task_id: Uuid",
                       container_ref,
                       branch,
                       setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                       created_at        AS "created_at!: DateTime<Utc>",
                       updated_at        AS "updated_at!: DateTime<Utc>",
                       archived          AS "archived!: bool",
                       pinned            AS "pinned!: bool",
                       name,
                       worktree_deleted  AS "worktree_deleted!: bool",
                       current_pipeline_stage,
                       speckit_feature_key,
                       speckit_host_repo_id AS "speckit_host_repo_id: Uuid"
               FROM    workspaces
               WHERE   rowid = $1"#,
            rowid
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn container_ref_exists(
        pool: &SqlitePool,
        container_ref: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM workspaces WHERE container_ref = ?) as "exists!: bool""#,
            container_ref
        )
        .fetch_one(pool)
        .await?;

        Ok(result.exists)
    }

    /// Find workspaces that are expired and eligible for cleanup.
    /// Uses accelerated cleanup (1 hour) for archived workspaces.
    /// Uses standard cleanup (72 hours) for non-archived workspaces.
    pub async fn find_expired_for_cleanup(
        pool: &SqlitePool,
    ) -> Result<Vec<Workspace>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                w.id as "id!: Uuid",
                w.task_id as "task_id: Uuid",
                w.container_ref,
                w.branch as "branch!",
                w.setup_completed_at as "setup_completed_at: DateTime<Utc>",
                w.created_at as "created_at!: DateTime<Utc>",
                w.updated_at as "updated_at!: DateTime<Utc>",
                w.archived as "archived!: bool",
                w.pinned as "pinned!: bool",
                w.name,
                w.worktree_deleted as "worktree_deleted!: bool",
                w.current_pipeline_stage,
                w.speckit_feature_key,
                w.speckit_host_repo_id as "speckit_host_repo_id: Uuid"
            FROM workspaces w
            LEFT JOIN sessions s ON w.id = s.workspace_id
            LEFT JOIN execution_processes ep ON s.id = ep.session_id AND ep.completed_at IS NOT NULL
            WHERE w.container_ref IS NOT NULL
                AND w.worktree_deleted = FALSE
                AND w.id NOT IN (
                    SELECT DISTINCT s2.workspace_id
                    FROM sessions s2
                    JOIN execution_processes ep2 ON s2.id = ep2.session_id
                    WHERE ep2.completed_at IS NULL
                )
            GROUP BY w.id, w.container_ref, w.updated_at
            HAVING datetime('now', 'localtime',
                CASE
                    WHEN w.archived = 1
                    THEN '-1 hours'
                    ELSE '-72 hours'
                END
            ) > datetime(
                MAX(
                    max(
                        datetime(w.updated_at),
                        datetime(ep.completed_at)
                    )
                )
            )
            ORDER BY MAX(
                CASE
                    WHEN ep.completed_at IS NOT NULL THEN ep.completed_at
                    ELSE w.updated_at
                END
            ) ASC
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateWorkspace,
        id: Uuid,
    ) -> Result<Self, WorkspaceError> {
        Ok(sqlx::query_as!(
            Workspace,
            r#"INSERT INTO workspaces (id, task_id, container_ref, branch, setup_completed_at, name)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id as "id!: Uuid", task_id as "task_id: Uuid", container_ref, branch, setup_completed_at as "setup_completed_at: DateTime<Utc>", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", archived as "archived!: bool", pinned as "pinned!: bool", name, worktree_deleted as "worktree_deleted!: bool", current_pipeline_stage, speckit_feature_key, speckit_host_repo_id as "speckit_host_repo_id: Uuid""#,
            id,
            Option::<Uuid>::None,
            Option::<String>::None,
            data.branch,
            Option::<DateTime<Utc>>::None,
            data.name
        )
        .fetch_one(pool)
        .await?)
    }

    pub async fn update_branch_name(
        pool: &SqlitePool,
        workspace_id: Uuid,
        new_branch_name: &str,
    ) -> Result<(), WorkspaceError> {
        sqlx::query!(
            "UPDATE workspaces SET branch = $1, updated_at = datetime('now') WHERE id = $2",
            new_branch_name,
            workspace_id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find workspace by path using container-ref path containment.
    /// Used by clients that may open a repo subfolder rather than the workspace root.
    pub async fn resolve_container_ref_by_prefix(
        pool: &SqlitePool,
        path: &str,
    ) -> Result<ContainerInfo, sqlx::Error> {
        let workspaces = sqlx::query_as!(
            WorkspaceContainerRefRow,
            r#"SELECT id as "id!: Uuid",
                      container_ref as "container_ref!"
               FROM workspaces
               WHERE container_ref IS NOT NULL"#,
        )
        .fetch_all(pool)
        .await?;

        Self::best_matching_container_ref(
            path,
            workspaces
                .iter()
                .map(|ws| (ws.id, ws.container_ref.as_str())),
        )
        .map(|workspace_id| ContainerInfo { workspace_id })
        .ok_or(sqlx::Error::RowNotFound)
    }

    fn best_matching_container_ref<'a>(
        path: &str,
        candidates: impl Iterator<Item = (Uuid, &'a str)>,
    ) -> Option<Uuid> {
        let path = std::path::Path::new(path);

        candidates
            .filter(|(_, container_ref)| {
                let container_ref = std::path::Path::new(container_ref);
                path.starts_with(container_ref) || container_ref.starts_with(path)
            })
            .max_by_key(|(_, container_ref)| {
                std::path::Path::new(container_ref).components().count()
            })
            .map(|(workspace_id, _)| workspace_id)
    }

    pub async fn set_archived(
        pool: &SqlitePool,
        workspace_id: Uuid,
        archived: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET archived = $1, updated_at = datetime('now', 'subsec') WHERE id = $2",
            archived,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update workspace fields. Only non-None values will be updated.
    /// For `name`, pass `Some("")` to clear the name, `Some("foo")` to set it, or `None` to leave unchanged.
    pub async fn update(
        pool: &SqlitePool,
        workspace_id: Uuid,
        archived: Option<bool>,
        pinned: Option<bool>,
        name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        // Convert empty string to None for name field (to store as NULL)
        let name_value = name.filter(|s| !s.is_empty());
        let name_provided = name.is_some();

        sqlx::query!(
            r#"UPDATE workspaces SET
                archived = COALESCE($1, archived),
                pinned = COALESCE($2, pinned),
                name = CASE WHEN $3 THEN $4 ELSE name END,
                updated_at = datetime('now', 'subsec')
            WHERE id = $5"#,
            archived,
            pinned,
            name_provided,
            name_value,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remote project this workspace is linked to, if any. Kept out of the
    /// `Workspace` struct (and its generated TS type) on purpose: only the
    /// org-env-var resolution on the spawn path needs it.
    pub async fn get_remote_project_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT remote_project_id as "remote_project_id: Uuid"
               FROM workspaces WHERE id = $1"#,
            workspace_id
        )
        .fetch_optional(pool)
        .await
        .map(Option::flatten)
    }

    /// Record (or clear, with `None`) the remote project a workspace is
    /// linked to.
    pub async fn set_remote_project_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
        remote_project_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET remote_project_id = $1, updated_at = datetime('now', 'subsec') WHERE id = $2",
            remote_project_id,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Persist the workspace's currently-reported pipeline stage (1-based,
    /// `None` = not yet reported / reset for a new coding-agent run).
    /// Single source of truth for `VK-PIPELINE-STAGE` marker detection.
    pub async fn set_current_pipeline_stage(
        pool: &SqlitePool,
        workspace_id: Uuid,
        stage: Option<i64>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET current_pipeline_stage = $1, updated_at = datetime('now', 'subsec') WHERE id = $2",
            stage,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// The most recent workspace for a task whose worktree has not been
    /// deleted — the workspace whose on-disk SpecKit artifacts the viewer
    /// should read.
    pub async fn find_latest_by_task_id(
        pool: &SqlitePool,
        task_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT  id                AS "id!: Uuid",
                       task_id           AS "task_id: Uuid",
                       container_ref,
                       branch,
                       setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                       created_at        AS "created_at!: DateTime<Utc>",
                       updated_at        AS "updated_at!: DateTime<Utc>",
                       archived          AS "archived!: bool",
                       pinned            AS "pinned!: bool",
                       name,
                       worktree_deleted  AS "worktree_deleted!: bool",
                       current_pipeline_stage,
                       speckit_feature_key,
                       speckit_host_repo_id AS "speckit_host_repo_id: Uuid"
               FROM    workspaces
               WHERE   task_id = $1 AND worktree_deleted = FALSE
               ORDER BY created_at DESC
               LIMIT 1"#,
            task_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Persist the SpecKit host repo + feature key, but only at *first*
    /// provisioning: the guard (`speckit_feature_key IS NULL`) makes this a
    /// no-op once a feature key has ever been recorded, so the key and host
    /// stay stable across branch renames and repo additions. Returns whether
    /// the row was updated.
    pub async fn set_speckit_provisioning(
        pool: &SqlitePool,
        workspace_id: Uuid,
        host_repo_id: Uuid,
        feature_key: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"UPDATE workspaces
               SET speckit_host_repo_id = $1,
                   speckit_feature_key = $2,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $3 AND speckit_feature_key IS NULL"#,
            host_repo_id,
            feature_key,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_first_user_message(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let actions = sqlx::query_scalar!(
            r#"SELECT ep.executor_action as "executor_action!: sqlx::types::Json<ExecutorActionField>"
               FROM sessions s
               JOIN execution_processes ep ON ep.session_id = s.id
               WHERE s.workspace_id = $1
               ORDER BY s.created_at ASC, ep.created_at ASC"#,
            workspace_id
        )
        .fetch_all(pool)
        .await?;

        for action in actions {
            if let ExecutorActionField::ExecutorAction(action) = action.0
                && let Some(prompt) = Self::extract_first_prompt_from_executor_action(&action)
            {
                return Ok(Some(prompt));
            }
        }

        Ok(None)
    }

    fn extract_first_prompt_from_executor_action(action: &ExecutorAction) -> Option<String> {
        let mut current = Some(action);
        while let Some(action) = current {
            match action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    return Some(request.prompt.clone());
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    return Some(request.prompt.clone());
                }
                ExecutorActionType::ReviewRequest(request) => {
                    return Some(request.prompt.clone());
                }
                ExecutorActionType::ScriptRequest(_) => {
                    current = action.next_action();
                }
            }
        }
        None
    }

    pub fn truncate_to_name(prompt: &str, max_len: usize) -> String {
        let trimmed = prompt.trim();
        if trimmed.chars().count() <= max_len {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(max_len).collect();
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }

    pub async fn find_all_with_status(
        pool: &SqlitePool,
        archived: Option<bool>,
        limit: Option<i64>,
    ) -> Result<Vec<WorkspaceWithStatus>, sqlx::Error> {
        // Fetch all workspaces with status (uses cached SQLx query)
        let records = sqlx::query!(
            r#"SELECT
                w.id AS "id!: Uuid",
                w.task_id AS "task_id: Uuid",
                w.container_ref,
                w.branch,
                w.setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                w.created_at AS "created_at!: DateTime<Utc>",
                w.updated_at AS "updated_at!: DateTime<Utc>",
                w.archived AS "archived!: bool",
                w.pinned AS "pinned!: bool",
                w.name,
                w.worktree_deleted AS "worktree_deleted!: bool",
                w.current_pipeline_stage,
                w.speckit_feature_key,
                w.speckit_host_repo_id AS "speckit_host_repo_id: Uuid",

                CASE WHEN EXISTS (
                    SELECT 1
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.status = 'running'
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    LIMIT 1
                ) THEN 1 ELSE 0 END AS "is_running!: i64",

                CASE WHEN (
                    SELECT ep.status
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    ORDER BY ep.created_at DESC
                    LIMIT 1
                ) IN ('failed','killed') THEN 1 ELSE 0 END AS "is_errored!: i64"

            FROM workspaces w
            WHERE ($1 IS NULL OR w.archived = $1)
            ORDER BY w.updated_at DESC
            LIMIT COALESCE($2, -1)"#,
            archived,
            limit
        )
        .fetch_all(pool)
        .await?;

        let mut workspaces: Vec<WorkspaceWithStatus> = records
            .into_iter()
            .map(|rec| WorkspaceWithStatus {
                workspace: Workspace {
                    id: rec.id,
                    task_id: rec.task_id,
                    container_ref: rec.container_ref,
                    branch: rec.branch,
                    setup_completed_at: rec.setup_completed_at,
                    created_at: rec.created_at,
                    updated_at: rec.updated_at,
                    archived: rec.archived,
                    pinned: rec.pinned,
                    name: rec.name,
                    worktree_deleted: rec.worktree_deleted,
                    current_pipeline_stage: rec.current_pipeline_stage,
                    speckit_feature_key: rec.speckit_feature_key,
                    speckit_host_repo_id: rec.speckit_host_repo_id,
                },
                is_running: rec.is_running != 0,
                is_errored: rec.is_errored != 0,
            })
            .collect();

        for ws in &mut workspaces {
            if ws.workspace.name.is_none()
                && let Some(prompt) = Self::get_first_user_message(pool, ws.workspace.id).await?
            {
                let name = Self::truncate_to_name(&prompt, WORKSPACE_NAME_MAX_LEN);
                Self::update(pool, ws.workspace.id, None, None, Some(&name)).await?;
                ws.workspace.name = Some(name);
            }
        }

        Ok(workspaces)
    }

    /// Delete a workspace by ID
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM workspaces WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Count total workspaces across all projects
    pub async fn find_by_id_with_status(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<Option<WorkspaceWithStatus>, sqlx::Error> {
        let rec = sqlx::query!(
            r#"SELECT
                w.id AS "id!: Uuid",
                w.task_id AS "task_id: Uuid",
                w.container_ref,
                w.branch,
                w.setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                w.created_at AS "created_at!: DateTime<Utc>",
                w.updated_at AS "updated_at!: DateTime<Utc>",
                w.archived AS "archived!: bool",
                w.pinned AS "pinned!: bool",
                w.name,
                w.worktree_deleted AS "worktree_deleted!: bool",
                w.current_pipeline_stage,
                w.speckit_feature_key,
                w.speckit_host_repo_id AS "speckit_host_repo_id: Uuid",

                CASE WHEN EXISTS (
                    SELECT 1
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.status = 'running'
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    LIMIT 1
                ) THEN 1 ELSE 0 END AS "is_running!: i64",

                CASE WHEN (
                    SELECT ep.status
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    ORDER BY ep.created_at DESC
                    LIMIT 1
                ) IN ('failed','killed') THEN 1 ELSE 0 END AS "is_errored!: i64"

            FROM workspaces w
            WHERE w.id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        let mut ws = WorkspaceWithStatus {
            workspace: Workspace {
                id: rec.id,
                task_id: rec.task_id,
                container_ref: rec.container_ref,
                branch: rec.branch,
                setup_completed_at: rec.setup_completed_at,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
                archived: rec.archived,
                pinned: rec.pinned,
                name: rec.name,
                worktree_deleted: rec.worktree_deleted,
                current_pipeline_stage: rec.current_pipeline_stage,
                speckit_feature_key: rec.speckit_feature_key,
                speckit_host_repo_id: rec.speckit_host_repo_id,
            },
            is_running: rec.is_running != 0,
            is_errored: rec.is_errored != 0,
        };

        if ws.workspace.name.is_none()
            && let Some(prompt) = Self::get_first_user_message(pool, ws.workspace.id).await?
        {
            let name = Self::truncate_to_name(&prompt, WORKSPACE_NAME_MAX_LEN);
            Self::update(pool, ws.workspace.id, None, None, Some(&name)).await?;
            ws.workspace.name = Some(name);
        }

        Ok(Some(ws))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::{CreateWorkspace, Workspace, WorkspacePlacement, WorkspacePlacementState};
    use crate::models::worker_node::{UpsertWorkerNode, WorkerMountStatus, WorkerNode};

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    /// `find_all_with_status` used to select every row and then filter `archived`
    /// and truncate to `limit` in Rust. Both are now pushed into SQL, where the
    /// `archived` bind is referenced twice (`$1 IS NULL OR w.archived = $1`) and
    /// the limit goes through `LIMIT COALESCE($2, -1)`. Neither is obviously
    /// correct by inspection, so pin all three filter cases and the limit here.
    #[tokio::test]
    async fn find_all_with_status_filters_and_limits_in_sql() {
        let pool = test_pool().await;

        let mut active_ids = Vec::new();
        for i in 0..3 {
            let ws = Workspace::create(
                &pool,
                &CreateWorkspace {
                    branch: format!("vk/active-{i}"),
                    name: Some(format!("active-{i}")),
                },
                Uuid::new_v4(),
            )
            .await
            .unwrap();
            active_ids.push(ws.id);
        }

        let mut archived_ids = Vec::new();
        for i in 0..2 {
            let ws = Workspace::create(
                &pool,
                &CreateWorkspace {
                    branch: format!("vk/archived-{i}"),
                    name: Some(format!("archived-{i}")),
                },
                Uuid::new_v4(),
            )
            .await
            .unwrap();
            Workspace::set_archived(&pool, ws.id, true).await.unwrap();
            archived_ids.push(ws.id);
        }

        let ids = |rows: Vec<super::WorkspaceWithStatus>| -> Vec<Uuid> {
            rows.into_iter().map(|r| r.workspace.id).collect()
        };

        // archived = false -> only the active ones.
        let actives = ids(Workspace::find_all_with_status(&pool, Some(false), None)
            .await
            .unwrap());
        assert_eq!(actives.len(), 3);
        for id in &active_ids {
            assert!(actives.contains(id), "missing active {id}");
        }
        for id in &archived_ids {
            assert!(!actives.contains(id), "archived {id} leaked into actives");
        }

        // archived = true -> only the archived ones.
        let archiveds = ids(Workspace::find_all_with_status(&pool, Some(true), None)
            .await
            .unwrap());
        assert_eq!(archiveds.len(), 2);
        for id in &archived_ids {
            assert!(archiveds.contains(id), "missing archived {id}");
        }

        // None -> no filter at all, i.e. the `$1 IS NULL` branch.
        let all = ids(Workspace::find_all_with_status(&pool, None, None)
            .await
            .unwrap());
        assert_eq!(all.len(), 5);

        // Ordering is `updated_at DESC`, and the limit must respect it rather
        // than returning an arbitrary subset.
        let ordered = ids(Workspace::find_all_with_status(&pool, None, None)
            .await
            .unwrap());
        let limited = ids(Workspace::find_all_with_status(&pool, None, Some(2))
            .await
            .unwrap());
        assert_eq!(limited, ordered[..2].to_vec());

        // A limit larger than the row count is not an error.
        assert_eq!(
            ids(Workspace::find_all_with_status(&pool, None, Some(99))
                .await
                .unwrap())
            .len(),
            5
        );

        // A limit combined with a filter applies to the filtered set.
        assert_eq!(
            ids(Workspace::find_all_with_status(&pool, Some(false), Some(1))
                .await
                .unwrap())
            .len(),
            1
        );
    }

    #[tokio::test]
    async fn remote_project_id_round_trips_and_clears() {
        let pool = test_pool().await;
        let ws = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "vk/remote-project-id-test".to_string(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();

        // New workspaces start unlinked.
        assert_eq!(
            Workspace::get_remote_project_id(&pool, ws.id)
                .await
                .unwrap(),
            None
        );

        let remote_project_id = Uuid::new_v4();
        Workspace::set_remote_project_id(&pool, ws.id, Some(remote_project_id))
            .await
            .unwrap();
        assert_eq!(
            Workspace::get_remote_project_id(&pool, ws.id)
                .await
                .unwrap(),
            Some(remote_project_id)
        );

        // Unlinking clears the stored id.
        Workspace::set_remote_project_id(&pool, ws.id, None)
            .await
            .unwrap();
        assert_eq!(
            Workspace::get_remote_project_id(&pool, ws.id)
                .await
                .unwrap(),
            None
        );

        // Unknown workspaces resolve to None rather than erroring.
        assert_eq!(
            Workspace::get_remote_project_id(&pool, Uuid::new_v4())
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn placement_is_sticky_and_transitions_forward() {
        let pool = test_pool().await;
        let workspace = Workspace::create(
            &pool,
            &CreateWorkspace {
                branch: "vk/cluster-placement-test".to_string(),
                name: None,
            },
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        let now = Utc::now();
        let worker_id = Uuid::new_v4();
        WorkerNode::upsert_heartbeat(
            &pool,
            &UpsertWorkerNode {
                id: worker_id,
                hostname: "think3".into(),
                worker_version: "1".into(),
                vibe_version: "1".into(),
                capabilities: serde_json::json!({}),
                resource_snapshot: serde_json::json!({}),
                labels: serde_json::json!({}),
                mount_status: WorkerMountStatus::Healthy,
                mount_message: None,
                heartbeat_at: now,
                lease_expires_at: now + chrono::Duration::seconds(30),
            },
        )
        .await
        .unwrap();

        assert!(
            WorkspacePlacement::reserve(
                &pool,
                workspace.id,
                worker_id,
                None,
                None,
                Some("automatic"),
            )
            .await
            .unwrap()
        );
        assert!(
            !WorkspacePlacement::reserve(&pool, workspace.id, Uuid::new_v4(), None, None, None,)
                .await
                .unwrap()
        );
        assert!(
            WorkspacePlacement::transition(
                &pool,
                workspace.id,
                WorkspacePlacementState::Reserved,
                WorkspacePlacementState::Provisioning,
                None,
            )
            .await
            .unwrap()
        );
        assert!(
            WorkspacePlacement::transition(
                &pool,
                workspace.id,
                WorkspacePlacementState::Provisioning,
                WorkspacePlacementState::Ready,
                Some("provisioned"),
            )
            .await
            .unwrap()
        );

        let placement = WorkspacePlacement::find(&pool, workspace.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(placement.worker_node_id, Some(worker_id));
        assert_eq!(placement.placement_state, WorkspacePlacementState::Ready);
        assert_eq!(placement.placement_reason.as_deref(), Some("provisioned"));
    }

    #[test]
    fn best_matching_container_ref_prefers_deepest_match() {
        let broad_id = Uuid::new_v4();
        let exact_id = Uuid::new_v4();
        let selected = Workspace::best_matching_container_ref(
            "/tmp/ws/repo/packages/app",
            [(broad_id, "/tmp"), (exact_id, "/tmp/ws")].into_iter(),
        );

        assert_eq!(selected, Some(exact_id));
    }

    #[test]
    fn best_matching_container_ref_supports_parent_request_path() {
        let workspace_id = Uuid::new_v4();
        let selected = Workspace::best_matching_container_ref(
            "/tmp/ws/repo",
            [(workspace_id, "/tmp/ws/repo/packages/app")].into_iter(),
        );

        assert_eq!(selected, Some(workspace_id));
    }

    #[test]
    fn best_matching_container_ref_ignores_unrelated_paths() {
        let workspace_id = Uuid::new_v4();
        let selected = Workspace::best_matching_container_ref(
            "/tmp/other/path",
            [(workspace_id, "/tmp/ws")].into_iter(),
        );

        assert_eq!(selected, None);
    }
}
