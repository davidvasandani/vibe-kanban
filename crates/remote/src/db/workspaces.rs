use api_types::Workspace;
use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct CreateWorkspaceParams {
    pub project_id: Uuid,
    pub owner_user_id: Uuid,
    pub local_workspace_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub name: Option<String>,
    pub archived: Option<bool>,
    pub files_changed: Option<i32>,
    pub lines_added: Option<i32>,
    pub lines_removed: Option<i32>,
}

pub struct WorkspaceRepository;

impl WorkspaceRepository {
    pub async fn list_by_owner(
        pool: &PgPool,
        owner_user_id: Uuid,
    ) -> Result<Vec<Workspace>, WorkspaceError> {
        let records = sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM workspaces
            WHERE owner_user_id = $1
            "#,
            owner_user_id
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<Workspace>, WorkspaceError> {
        let records = sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM workspaces
            WHERE project_id = $1
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    pub async fn create(
        pool: &PgPool,
        params: CreateWorkspaceParams,
    ) -> Result<Workspace, WorkspaceError> {
        let CreateWorkspaceParams {
            project_id,
            owner_user_id,
            local_workspace_id,
            issue_id,
            name,
            archived,
            files_changed,
            lines_added,
            lines_removed,
        } = params;
        let archived = archived.unwrap_or(false);
        let record = sqlx::query_as!(
            Workspace,
            r#"
            INSERT INTO workspaces (project_id, owner_user_id, local_workspace_id, issue_id, name, archived, files_changed, lines_added, lines_removed)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            project_id,
            owner_user_id,
            local_workspace_id,
            issue_id,
            name,
            archived,
            files_changed,
            lines_added,
            lines_removed
        )
        .fetch_one(pool)
        .await?;
        Ok(record)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Workspace>, WorkspaceError> {
        let record = sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM workspaces
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_local_id(
        pool: &PgPool,
        local_workspace_id: Uuid,
    ) -> Result<Option<Workspace>, WorkspaceError> {
        let record = sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM workspaces
            WHERE local_workspace_id = $1
            "#,
            local_workspace_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn exists_by_local_id(
        pool: &PgPool,
        local_workspace_id: Uuid,
    ) -> Result<bool, WorkspaceError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM workspaces WHERE local_workspace_id = $1) AS "exists!""#,
            local_workspace_id
        )
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    pub async fn delete_by_local_id(
        pool: &PgPool,
        local_workspace_id: Uuid,
    ) -> Result<(), WorkspaceError> {
        sqlx::query!(
            "DELETE FROM workspaces WHERE local_workspace_id = $1",
            local_workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), WorkspaceError> {
        sqlx::query!("DELETE FROM workspaces WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn count_by_issue_id(pool: &PgPool, issue_id: Uuid) -> Result<i64, WorkspaceError> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM workspaces WHERE issue_id = $1"#,
            issue_id
        )
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    /// Lists the non-archived workspaces linked to an issue. Generic over the
    /// executor so it can run inside an existing transaction.
    pub async fn list_active_by_issue_id<'e, E>(
        executor: E,
        issue_id: Uuid,
    ) -> Result<Vec<Workspace>, WorkspaceError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let records = sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM workspaces
            WHERE issue_id = $1 AND archived = FALSE
            "#,
            issue_id
        )
        .fetch_all(executor)
        .await?;
        Ok(records)
    }

    /// Archives all non-archived workspaces linked to an issue, returning the
    /// number of workspaces archived. Generic over the executor so it can run
    /// inside an existing transaction.
    pub async fn archive_active_by_issue_id<'e, E>(
        executor: E,
        issue_id: Uuid,
    ) -> Result<u64, WorkspaceError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let result = sqlx::query!(
            r#"
            UPDATE workspaces
            SET archived = TRUE, updated_at = NOW()
            WHERE issue_id = $1 AND archived = FALSE
            "#,
            issue_id
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<Option<String>>,
        archived: Option<bool>,
        files_changed: Option<Option<i32>>,
        lines_added: Option<Option<i32>>,
        lines_removed: Option<Option<i32>>,
    ) -> Result<Workspace, WorkspaceError> {
        let mut tx = pool.begin().await?;

        // Issue mutations lock the issue before archiving its workspaces. Use
        // the same order here so concurrent Done/unarchive cannot deadlock.
        if archived == Some(false) {
            sqlx::query(
                "SELECT id FROM issues WHERE id = (SELECT issue_id FROM workspaces WHERE id = $1) FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        }
        let was_archived: bool =
            sqlx::query_scalar("SELECT archived FROM workspaces WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;

        let update_name = name.is_some();
        let name_value = name.flatten();

        let update_archived = archived.is_some();
        let archived_value = archived.unwrap_or(false);

        let update_files_changed = files_changed.is_some();
        let files_changed_value = files_changed.flatten();

        let update_lines_added = lines_added.is_some();
        let lines_added_value = lines_added.flatten();

        let update_lines_removed = lines_removed.is_some();
        let lines_removed_value = lines_removed.flatten();

        let record = sqlx::query_as!(
            Workspace,
            r#"
            UPDATE workspaces SET
                name = CASE WHEN $1 THEN $2 ELSE name END,
                archived = CASE WHEN $3 THEN $4 ELSE archived END,
                files_changed = CASE WHEN $5 THEN $6 ELSE files_changed END,
                lines_added = CASE WHEN $7 THEN $8 ELSE lines_added END,
                lines_removed = CASE WHEN $9 THEN $10 ELSE lines_removed END,
                updated_at = NOW()
            WHERE id = $11
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                owner_user_id       AS "owner_user_id!: Uuid",
                issue_id            AS "issue_id: Uuid",
                local_workspace_id  AS "local_workspace_id: Uuid",
                name                AS "name: String",
                archived            AS "archived!: bool",
                files_changed       AS "files_changed: i32",
                lines_added         AS "lines_added: i32",
                lines_removed       AS "lines_removed: i32",
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            update_name,
            name_value,
            update_archived,
            archived_value,
            update_files_changed,
            files_changed_value,
            update_lines_added,
            lines_added_value,
            update_lines_removed,
            lines_removed_value,
            id
        )
        .fetch_one(&mut *tx)
        .await?;

        if was_archived && !record.archived {
            // Match ProjectStatusRepository::find_by_name: project-scoped,
            // case-insensitive names. A missing target leaves the issue alone.
            sqlx::query(include_str!("sql/reopen_workspace_issue.sql"))
                .bind(record.issue_id)
                .bind(record.project_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(record)
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    // Minimal relational fixture keeps the tests independent of auth and Electric.
    async fn fixture(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
        sqlx::raw_sql(
            "CREATE TABLE project_statuses (id uuid PRIMARY KEY, project_id uuid, name text);
             CREATE TABLE issues (id uuid PRIMARY KEY, project_id uuid, status_id uuid,
                                  updated_at timestamptz DEFAULT now());
             CREATE TABLE workspaces (
                 id uuid PRIMARY KEY, project_id uuid NOT NULL, owner_user_id uuid NOT NULL,
                 issue_id uuid, local_workspace_id uuid, name text, archived boolean NOT NULL,
                 files_changed integer, lines_added integer, lines_removed integer,
                 created_at timestamptz NOT NULL DEFAULT now(),
                 updated_at timestamptz NOT NULL DEFAULT now());",
        )
        .execute(pool)
        .await
        .unwrap();
        let project = Uuid::new_v4();
        let issue = Uuid::new_v4();
        let workspace = Uuid::new_v4();
        let done = Uuid::new_v4();
        let progress = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO project_statuses VALUES ($1, $3, 'dOnE'), ($2, $3, 'IN PROGRESS')",
        )
        .bind(done)
        .bind(progress)
        .bind(project)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO issues (id, project_id, status_id) VALUES ($1, $2, $3)")
            .bind(issue)
            .bind(project)
            .bind(done)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspaces (id, project_id, owner_user_id, issue_id, archived) VALUES ($1, $2, $3, $4, true)")
            .bind(workspace).bind(project).bind(Uuid::new_v4()).bind(issue).execute(pool).await.unwrap();
        (workspace, issue, done, progress)
    }

    async fn status(pool: &PgPool, issue: Uuid) -> Uuid {
        sqlx::query_scalar("SELECT status_id FROM issues WHERE id = $1")
            .bind(issue)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn update(
        pool: &PgPool,
        workspace: Uuid,
        archived: Option<bool>,
    ) -> Result<Workspace, WorkspaceError> {
        WorkspaceRepository::update(
            pool,
            workspace,
            Some(Some("renamed".into())),
            archived,
            None,
            None,
            None,
        )
        .await
    }

    #[sqlx::test(migrations = false)]
    async fn reactivation_reopens_done_only_on_transition(pool: PgPool) {
        let (workspace, issue, done, progress) = fixture(&pool).await;
        assert!(update(&pool, workspace, None).await.unwrap().archived);
        assert_eq!(status(&pool, issue).await, done);
        assert!(update(&pool, workspace, Some(true)).await.unwrap().archived);
        assert_eq!(status(&pool, issue).await, done);
        assert!(
            !update(&pool, workspace, Some(false))
                .await
                .unwrap()
                .archived
        );
        assert_eq!(status(&pool, issue).await, progress);
        // An already-active update must not reopen a subsequently completed issue.
        sqlx::query("UPDATE issues SET status_id = $1 WHERE id = $2")
            .bind(done)
            .bind(issue)
            .execute(&pool)
            .await
            .unwrap();
        update(&pool, workspace, Some(false)).await.unwrap();
        assert_eq!(status(&pool, issue).await, done);
    }

    #[sqlx::test(migrations = false)]
    async fn reactivation_preserves_other_statuses_and_missing_target(pool: PgPool) {
        let (workspace, issue, done, progress) = fixture(&pool).await;
        for name in ["Cancelled", "Backlog", "To do", "In review", "Custom"] {
            sqlx::query("UPDATE project_statuses SET name = $1 WHERE id = $2")
                .bind(name)
                .bind(done)
                .execute(&pool)
                .await
                .unwrap();
            update(&pool, workspace, Some(true)).await.unwrap();
            update(&pool, workspace, Some(false)).await.unwrap();
            assert_eq!(status(&pool, issue).await, done);
        }
        sqlx::query("UPDATE project_statuses SET name = 'Done' WHERE id = $1")
            .bind(done)
            .execute(&pool)
            .await
            .unwrap();
        // A target in a different project must not be used.
        sqlx::query("UPDATE project_statuses SET project_id = $1 WHERE id = $2")
            .bind(Uuid::new_v4())
            .bind(progress)
            .execute(&pool)
            .await
            .unwrap();
        update(&pool, workspace, Some(true)).await.unwrap();
        update(&pool, workspace, Some(false)).await.unwrap();
        assert_eq!(status(&pool, issue).await, done);
        sqlx::query("UPDATE workspaces SET issue_id = NULL WHERE id = $1")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        update(&pool, workspace, Some(true)).await.unwrap();
        assert!(
            !update(&pool, workspace, Some(false))
                .await
                .unwrap()
                .archived
        );
    }

    #[sqlx::test(migrations = false)]
    async fn issue_failure_rolls_back_workspace_reactivation(pool: PgPool) {
        let (workspace, issue, done, progress) = fixture(&pool).await;
        sqlx::raw_sql(&format!("ALTER TABLE issues ADD CONSTRAINT reject_progress CHECK (status_id <> '{progress}'::uuid)"))
            .execute(&pool).await.unwrap();
        assert!(update(&pool, workspace, Some(false)).await.is_err());
        let archived: bool = sqlx::query_scalar("SELECT archived FROM workspaces WHERE id = $1")
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(archived);
        assert_eq!(status(&pool, issue).await, done);
    }

    #[sqlx::test(migrations = false)]
    async fn concurrent_reactivations_serialize(pool: PgPool) {
        let (workspace, issue, _, progress) = fixture(&pool).await;
        let (first, second) = tokio::join!(
            update(&pool, workspace, Some(false)),
            update(&pool, workspace, Some(false))
        );
        assert!(!first.unwrap().archived);
        assert!(!second.unwrap().archived);
        assert_eq!(status(&pool, issue).await, progress);
    }
}
