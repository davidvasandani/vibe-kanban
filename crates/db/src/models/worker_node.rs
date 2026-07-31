use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqlitePool, Type, types::Json};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Type, TS)]
#[sqlx(type_name = "worker_node_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum WorkerNodeStatus {
    Online,
    Offline,
    Draining,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Type, TS)]
#[sqlx(type_name = "worker_mount_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[ts(use_ts_enum)]
pub enum WorkerMountStatus {
    Healthy,
    Missing,
    LocalFallback,
    WrongFilesystem,
    ProbeNotVisible,
    ReadOnly,
    OwnershipMismatch,
    IoError,
}

impl WorkerMountStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct WorkerNode {
    pub id: Uuid,
    pub hostname: String,
    pub status: WorkerNodeStatus,
    pub worker_version: String,
    pub vibe_version: String,
    #[ts(type = "unknown")]
    pub capabilities: Json<Value>,
    #[ts(type = "unknown")]
    pub resource_snapshot: Json<Value>,
    #[ts(type = "unknown")]
    pub labels: Json<Value>,
    pub mount_status: WorkerMountStatus,
    pub mount_message: Option<String>,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct UpsertWorkerNode {
    pub id: Uuid,
    pub hostname: String,
    pub worker_version: String,
    pub vibe_version: String,
    pub capabilities: Value,
    pub resource_snapshot: Value,
    pub labels: Value,
    pub mount_status: WorkerMountStatus,
    pub mount_message: Option<String>,
    pub heartbeat_at: DateTime<Utc>,
    pub lease_expires_at: DateTime<Utc>,
}

impl WorkerNode {
    const SELECT: &'static str = r#"
        SELECT id, hostname, status, worker_version, vibe_version, capabilities,
               resource_snapshot, labels, mount_status, mount_message,
               last_heartbeat_at, lease_expires_at, created_at, updated_at
        FROM worker_nodes
    "#;

    pub async fn fetch_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} ORDER BY hostname, id", Self::SELECT))
            .fetch_all(pool)
            .await
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as::<_, Self>(&format!("{} WHERE id = ?", Self::SELECT))
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn upsert_heartbeat(
        pool: &SqlitePool,
        worker: &UpsertWorkerNode,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO worker_nodes (
                id, hostname, status, worker_version, vibe_version, capabilities,
                resource_snapshot, labels, mount_status, mount_message,
                last_heartbeat_at, lease_expires_at
            ) VALUES (?, ?, 'online', ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                hostname = excluded.hostname,
                status = CASE
                    WHEN worker_nodes.status = 'draining' THEN 'draining'
                    ELSE 'online'
                END,
                worker_version = excluded.worker_version,
                vibe_version = excluded.vibe_version,
                capabilities = excluded.capabilities,
                resource_snapshot = excluded.resource_snapshot,
                labels = excluded.labels,
                mount_status = excluded.mount_status,
                mount_message = excluded.mount_message,
                last_heartbeat_at = excluded.last_heartbeat_at,
                lease_expires_at = excluded.lease_expires_at,
                updated_at = datetime('now', 'subsec')
            "#,
        )
        .bind(worker.id)
        .bind(&worker.hostname)
        .bind(&worker.worker_version)
        .bind(&worker.vibe_version)
        .bind(Json(&worker.capabilities))
        .bind(Json(&worker.resource_snapshot))
        .bind(Json(&worker.labels))
        .bind(&worker.mount_status)
        .bind(&worker.mount_message)
        .bind(worker.heartbeat_at)
        .bind(worker.lease_expires_at)
        .execute(pool)
        .await?;

        Self::find_by_id(pool, worker.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn set_draining(
        pool: &SqlitePool,
        id: Uuid,
        draining: bool,
    ) -> Result<bool, sqlx::Error> {
        let status = if draining { "draining" } else { "offline" };
        let result = sqlx::query(
            "UPDATE worker_nodes SET status = ?, updated_at = datetime('now', 'subsec') WHERE id = ?",
        )
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn expire_leases(pool: &SqlitePool, now: DateTime<Utc>) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE worker_nodes
            SET status = 'offline', updated_at = datetime('now', 'subsec')
            WHERE status = 'online'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
            "#,
        )
        .bind(now)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn heartbeat_upsert_preserves_drain_and_expiry_skips_it() {
        let pool = test_pool().await;
        let now = Utc::now();
        let id = Uuid::new_v4();
        let worker = UpsertWorkerNode {
            id,
            hostname: "think3".into(),
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: serde_json::json!({"executors": ["codex"]}),
            resource_snapshot: serde_json::json!({"active_execution_count": 0}),
            labels: serde_json::json!({"pool": "think-cluster"}),
            mount_status: WorkerMountStatus::Healthy,
            mount_message: None,
            heartbeat_at: now,
            lease_expires_at: now + Duration::seconds(30),
        };

        assert_eq!(
            WorkerNode::upsert_heartbeat(&pool, &worker)
                .await
                .unwrap()
                .status,
            WorkerNodeStatus::Online
        );
        assert!(WorkerNode::set_draining(&pool, id, true).await.unwrap());
        assert_eq!(
            WorkerNode::upsert_heartbeat(&pool, &worker)
                .await
                .unwrap()
                .status,
            WorkerNodeStatus::Draining
        );
        assert_eq!(
            WorkerNode::expire_leases(&pool, now + Duration::minutes(1))
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn expired_online_worker_becomes_offline() {
        let pool = test_pool().await;
        let now = Utc::now();
        let id = Uuid::new_v4();
        WorkerNode::upsert_heartbeat(
            &pool,
            &UpsertWorkerNode {
                id,
                hostname: "think4".into(),
                worker_version: "1".into(),
                vibe_version: "1".into(),
                capabilities: serde_json::json!({}),
                resource_snapshot: serde_json::json!({}),
                labels: serde_json::json!({}),
                mount_status: WorkerMountStatus::Missing,
                mount_message: Some("mount missing".into()),
                heartbeat_at: now,
                lease_expires_at: now + Duration::seconds(5),
            },
        )
        .await
        .unwrap();

        assert_eq!(
            WorkerNode::expire_leases(&pool, now + Duration::seconds(6))
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            WorkerNode::find_by_id(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkerNodeStatus::Offline
        );
    }
}
