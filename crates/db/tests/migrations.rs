//! Round-trip tests for the execution_processes run_reason migration
//! (20260709000000_add_backgroundhelper_run_reason): the rebuilt column must
//! accept every enum variant, keep rejecting unknown values, and decode back
//! through the sqlx model layer.

use db::models::execution_process::{
    CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
};
use executors::actions::{
    ExecutorAction, ExecutorActionType,
    script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use uuid::Uuid;

/// In-memory pool with all migrations applied. Foreign keys are disabled so
/// rows can be inserted without their full parent chain (workspace/session).
async fn migrated_pool() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect to in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    // Some migrations re-enable foreign_keys on the (single, reused)
    // connection; turn it back off so rows don't need a full parent chain.
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&pool)
        .await
        .expect("disable foreign keys");
    pool
}

fn helper_action() -> ExecutorAction {
    ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: "sleep 1".to_string(),
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::BackgroundHelper,
            working_dir: None,
        }),
        None,
    )
}

#[tokio::test]
async fn all_run_reasons_round_trip_through_migrated_schema() {
    let pool = migrated_pool().await;

    for run_reason in [
        ExecutionProcessRunReason::SetupScript,
        ExecutionProcessRunReason::CleanupScript,
        ExecutionProcessRunReason::ArchiveScript,
        ExecutionProcessRunReason::CodingAgent,
        ExecutionProcessRunReason::DevServer,
        ExecutionProcessRunReason::BackgroundHelper,
    ] {
        let process_id = Uuid::new_v4();
        let created = ExecutionProcess::create(
            &pool,
            &CreateExecutionProcess {
                session_id: Uuid::new_v4(),
                executor_action: helper_action(),
                run_reason: run_reason.clone(),
            },
            process_id,
            &[],
        )
        .await
        .unwrap_or_else(|e| panic!("insert run_reason {run_reason:?}: {e}"));

        assert_eq!(created.run_reason, run_reason);
        assert_eq!(created.status, ExecutionProcessStatus::Running);

        let found = ExecutionProcess::find_by_id(&pool, process_id)
            .await
            .expect("query by id")
            .expect("row present");
        assert_eq!(found.run_reason, run_reason);
    }
}

#[tokio::test]
async fn check_constraint_rejects_unknown_run_reason() {
    let pool = migrated_pool().await;

    let result = sqlx::query(
        "INSERT INTO execution_processes (
             id, session_id, run_reason, executor_action, status,
             started_at, created_at, updated_at
         ) VALUES (?, ?, 'notareason', '{}', 'running',
                   datetime('now'), datetime('now'), datetime('now'))",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await;

    let err = result.expect_err("unknown run_reason must violate CHECK");
    assert!(
        err.to_string().contains("CHECK"),
        "expected CHECK constraint violation, got: {err}"
    );
}
