use chrono::{DateTime, TimeDelta, Utc};
use cluster_protocol::{
    CoordinatorLease, MountFailureReason, MountHealth, MountProbe, PROTOCOL_VERSION,
    WorkerHeartbeat, WorkerRegistration,
};
use db::models::worker_node::{UpsertWorkerNode, WorkerMountStatus, WorkerNode, WorkerNodeStatus};
use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

use super::ClusterConfig;

const MAX_MOUNT_ID_BYTES: usize = 256;
const MAX_MOUNT_MESSAGE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountChallenge {
    pub probe: MountProbe,
    pub expected_filesystem_id: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MountEvidenceError {
    #[error("mount evidence field {field} is empty or exceeds its size limit")]
    InvalidField { field: &'static str },
    #[error("worker reported probe {actual:?}, expected {expected:?}")]
    ProbeMismatch { expected: String, actual: String },
    #[error("worker reported filesystem {actual:?}, expected {expected:?}")]
    FilesystemMismatch { expected: String, actual: String },
}

#[derive(Debug, Error)]
pub enum WorkerRegistryError {
    #[error("cluster coordinator ID is not configured")]
    MissingCoordinatorId,
    #[error("unsupported worker protocol version {actual}; expected {expected}")]
    UnsupportedProtocol { actual: u16, expected: u16 },
    #[error("request is addressed to coordinator {actual}, expected {expected}")]
    WrongCoordinator { actual: Uuid, expected: Uuid },
    #[error("worker {0} is not registered")]
    WorkerNotRegistered(Uuid),
    #[error(transparent)]
    InvalidMountEvidence(#[from] MountEvidenceError),
    #[error("worker payload could not be persisted: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct WorkerRegistry {
    pool: SqlitePool,
    config: ClusterConfig,
}

impl WorkerRegistry {
    pub fn new(pool: SqlitePool, config: ClusterConfig) -> Self {
        Self { pool, config }
    }

    pub async fn register(
        &self,
        registration: &WorkerRegistration,
        challenge: &MountChallenge,
        received_at: DateTime<Utc>,
    ) -> Result<(WorkerNode, CoordinatorLease), WorkerRegistryError> {
        self.validate_authority(
            registration.authority.protocol_version,
            registration.authority.coordinator_id,
        )?;
        let (mount_status, mount_message) =
            validate_mount_evidence(&registration.mount, challenge)?;
        let lease_expires_at = self.lease_expires_at(received_at);
        let worker = WorkerNode::upsert_heartbeat(
            &self.pool,
            &UpsertWorkerNode {
                id: registration.authority.worker_node_id,
                hostname: registration.hostname.clone(),
                worker_version: registration.worker_version.clone(),
                vibe_version: registration.vibe_version.clone(),
                capabilities: serde_json::to_value(&registration.capabilities)?,
                resource_snapshot: serde_json::to_value(&registration.resources)?,
                labels: serde_json::to_value(&registration.labels)?,
                mount_status,
                mount_message,
                heartbeat_at: received_at,
                lease_expires_at,
            },
        )
        .await?;
        let lease = self.lease(&worker, challenge.probe.clone(), lease_expires_at);
        Ok((worker, lease))
    }

    pub async fn heartbeat(
        &self,
        heartbeat: &WorkerHeartbeat,
        challenge: &MountChallenge,
        received_at: DateTime<Utc>,
    ) -> Result<(WorkerNode, CoordinatorLease), WorkerRegistryError> {
        self.validate_authority(
            heartbeat.authority.protocol_version,
            heartbeat.authority.coordinator_id,
        )?;
        let id = heartbeat.authority.worker_node_id;
        let current = WorkerNode::find_by_id(&self.pool, id)
            .await?
            .ok_or(WorkerRegistryError::WorkerNotRegistered(id))?;
        let (mount_status, mount_message) = validate_mount_evidence(&heartbeat.mount, challenge)?;
        let lease_expires_at = self.lease_expires_at(received_at);
        let worker = WorkerNode::upsert_heartbeat(
            &self.pool,
            &UpsertWorkerNode {
                id,
                hostname: current.hostname,
                worker_version: current.worker_version,
                vibe_version: current.vibe_version,
                capabilities: current.capabilities.0,
                resource_snapshot: serde_json::to_value(&heartbeat.resources)?,
                labels: current.labels.0,
                mount_status,
                mount_message,
                heartbeat_at: received_at,
                lease_expires_at,
            },
        )
        .await?;
        let lease = self.lease(&worker, challenge.probe.clone(), lease_expires_at);
        Ok((worker, lease))
    }

    pub async fn set_draining(
        &self,
        worker_node_id: Uuid,
        draining: bool,
    ) -> Result<bool, WorkerRegistryError> {
        Ok(WorkerNode::set_draining(&self.pool, worker_node_id, draining).await?)
    }

    pub async fn expire_heartbeats(&self, now: DateTime<Utc>) -> Result<u64, WorkerRegistryError> {
        Ok(WorkerNode::expire_leases(&self.pool, now).await?)
    }

    fn validate_authority(
        &self,
        protocol_version: u16,
        coordinator_id: Uuid,
    ) -> Result<(), WorkerRegistryError> {
        if protocol_version != PROTOCOL_VERSION {
            return Err(WorkerRegistryError::UnsupportedProtocol {
                actual: protocol_version,
                expected: PROTOCOL_VERSION,
            });
        }
        let expected = self
            .config
            .coordinator_id
            .ok_or(WorkerRegistryError::MissingCoordinatorId)?;
        if coordinator_id != expected {
            return Err(WorkerRegistryError::WrongCoordinator {
                actual: coordinator_id,
                expected,
            });
        }
        Ok(())
    }

    fn lease_expires_at(&self, received_at: DateTime<Utc>) -> DateTime<Utc> {
        received_at
            + TimeDelta::from_std(self.config.lease_duration)
                .expect("validated lease duration must fit chrono")
    }

    fn lease(
        &self,
        worker: &WorkerNode,
        probe: MountProbe,
        lease_expires_at: DateTime<Utc>,
    ) -> CoordinatorLease {
        CoordinatorLease {
            accepted_protocol_version: PROTOCOL_VERSION,
            heartbeat_interval_seconds: self
                .config
                .heartbeat_interval
                .as_secs()
                .try_into()
                .expect("validated heartbeat interval must fit protocol"),
            lease_expires_at,
            draining: worker.status == WorkerNodeStatus::Draining,
            probe,
        }
    }
}

/// Pure, bounded conversion of worker evidence to scheduling state.
pub fn validate_mount_evidence(
    evidence: &MountHealth,
    challenge: &MountChallenge,
) -> Result<(WorkerMountStatus, Option<String>), MountEvidenceError> {
    bounded(
        "challenge probe ID",
        &challenge.probe.id,
        MAX_MOUNT_ID_BYTES,
    )?;
    bounded(
        "challenge filesystem ID",
        &challenge.expected_filesystem_id,
        MAX_MOUNT_ID_BYTES,
    )?;
    match evidence {
        MountHealth::Healthy {
            filesystem_id,
            probe_id,
        } => {
            bounded("filesystem ID", filesystem_id, MAX_MOUNT_ID_BYTES)?;
            bounded("probe ID", probe_id, MAX_MOUNT_ID_BYTES)?;
            if probe_id != &challenge.probe.id {
                return Err(MountEvidenceError::ProbeMismatch {
                    expected: challenge.probe.id.clone(),
                    actual: probe_id.clone(),
                });
            }
            if filesystem_id != &challenge.expected_filesystem_id {
                return Err(MountEvidenceError::FilesystemMismatch {
                    expected: challenge.expected_filesystem_id.clone(),
                    actual: filesystem_id.clone(),
                });
            }
            Ok((WorkerMountStatus::Healthy, None))
        }
        MountHealth::Unhealthy { reason, message } => {
            if message.len() > MAX_MOUNT_MESSAGE_BYTES {
                return Err(MountEvidenceError::InvalidField {
                    field: "mount diagnostic",
                });
            }
            Ok((
                failure_status(reason),
                (!message.is_empty()).then(|| message.clone()),
            ))
        }
    }
}

fn bounded(field: &'static str, value: &str, max: usize) -> Result<(), MountEvidenceError> {
    if value.is_empty() || value.len() > max {
        Err(MountEvidenceError::InvalidField { field })
    } else {
        Ok(())
    }
}

fn failure_status(reason: &MountFailureReason) -> WorkerMountStatus {
    match reason {
        MountFailureReason::Missing => WorkerMountStatus::Missing,
        MountFailureReason::LocalFallback => WorkerMountStatus::LocalFallback,
        MountFailureReason::WrongFilesystem => WorkerMountStatus::WrongFilesystem,
        MountFailureReason::ProbeNotVisible => WorkerMountStatus::ProbeNotVisible,
        MountFailureReason::ReadOnly => WorkerMountStatus::ReadOnly,
        MountFailureReason::OwnershipMismatch => WorkerMountStatus::OwnershipMismatch,
        MountFailureReason::IoError => WorkerMountStatus::IoError,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use cluster_protocol::{
        RequestAuthority, ResourceSnapshot, WorkerCapabilities, WorkerRegistration,
    };

    use super::*;

    fn challenge() -> MountChallenge {
        MountChallenge {
            probe: MountProbe {
                id: "probe-1".into(),
                relative_path: ".probes/probe-1".into(),
                expected_contents_digest: "sha256:abc".into(),
            },
            expected_filesystem_id: "nfs:shared".into(),
        }
    }

    fn healthy() -> MountHealth {
        MountHealth::Healthy {
            filesystem_id: "nfs:shared".into(),
            probe_id: "probe-1".into(),
        }
    }

    async fn registry() -> (WorkerRegistry, Uuid) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations")
            .run(&pool)
            .await
            .unwrap();
        let coordinator_id = Uuid::new_v4();
        (
            WorkerRegistry::new(
                pool,
                ClusterConfig {
                    enabled: true,
                    coordinator_id: Some(coordinator_id),
                    heartbeat_interval: Duration::from_secs(10),
                    lease_duration: Duration::from_secs(30),
                    ..ClusterConfig::default()
                },
            ),
            coordinator_id,
        )
    }

    fn registration(coordinator_id: Uuid, worker_node_id: Uuid) -> WorkerRegistration {
        WorkerRegistration {
            authority: RequestAuthority {
                protocol_version: PROTOCOL_VERSION,
                coordinator_id,
                worker_node_id,
                correlation_id: Uuid::new_v4(),
                issued_at: Utc::now(),
                nonce: "nonce".into(),
            },
            hostname: "think3".into(),
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: WorkerCapabilities::default(),
            resources: ResourceSnapshot {
                cpu_count: 8,
                load_1m: 0.5,
                available_memory_bytes: 1024,
                active_execution_count: 0,
            },
            labels: BTreeMap::new(),
            mount: healthy(),
        }
    }

    #[test]
    fn mount_evidence_is_exact_and_bounded() {
        assert_eq!(
            validate_mount_evidence(&healthy(), &challenge()).unwrap(),
            (WorkerMountStatus::Healthy, None)
        );
        let wrong = MountHealth::Healthy {
            filesystem_id: "local".into(),
            probe_id: "probe-1".into(),
        };
        assert!(matches!(
            validate_mount_evidence(&wrong, &challenge()),
            Err(MountEvidenceError::FilesystemMismatch { .. })
        ));
        assert!(matches!(
            validate_mount_evidence(
                &MountHealth::Unhealthy {
                    reason: MountFailureReason::IoError,
                    message: "x".repeat(MAX_MOUNT_MESSAGE_BYTES + 1),
                },
                &challenge()
            ),
            Err(MountEvidenceError::InvalidField { .. })
        ));
    }

    #[tokio::test]
    async fn heartbeat_preserves_drain_and_online_lease_expires() {
        let (registry, coordinator_id) = registry().await;
        let worker_id = Uuid::new_v4();
        let now = Utc::now();
        let registration = registration(coordinator_id, worker_id);
        let (worker, lease) = registry
            .register(&registration, &challenge(), now)
            .await
            .unwrap();
        assert_eq!(worker.status, WorkerNodeStatus::Online);
        assert_eq!(lease.lease_expires_at, now + TimeDelta::seconds(30));

        registry.set_draining(worker_id, true).await.unwrap();
        let heartbeat = WorkerHeartbeat {
            authority: registration.authority,
            resources: registration.resources,
            mount: healthy(),
            jobs: vec![],
        };
        let (worker, lease) = registry
            .heartbeat(&heartbeat, &challenge(), now + TimeDelta::seconds(10))
            .await
            .unwrap();
        assert_eq!(worker.status, WorkerNodeStatus::Draining);
        assert!(lease.draining);
        assert_eq!(
            registry
                .expire_heartbeats(now + TimeDelta::minutes(2))
                .await
                .unwrap(),
            0
        );

        registry.set_draining(worker_id, false).await.unwrap();
        registry
            .heartbeat(&heartbeat, &challenge(), now + TimeDelta::seconds(20))
            .await
            .unwrap();
        assert_eq!(
            registry
                .expire_heartbeats(now + TimeDelta::minutes(2))
                .await
                .unwrap(),
            1
        );
    }
}
