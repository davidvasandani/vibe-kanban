//! Transport-neutral contracts between a Vibe Kanban coordinator and worker.
//!
//! The coordinator remains authoritative for product state. These messages
//! describe worker evidence and commands; they do not grant the worker access
//! to the coordinator's database.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestAuthority {
    pub protocol_version: u16,
    pub coordinator_id: Uuid,
    pub worker_node_id: Uuid,
    pub correlation_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerRegistration {
    pub authority: RequestAuthority,
    pub hostname: String,
    pub worker_version: String,
    pub vibe_version: String,
    pub capabilities: WorkerCapabilities,
    pub resources: ResourceSnapshot,
    pub labels: BTreeMap<String, String>,
    pub mount: MountHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkerCapabilities {
    pub executor_profiles: Vec<String>,
    pub terminal: bool,
    pub preview: bool,
    pub persistent_process_adoption: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceSnapshot {
    pub cpu_count: u32,
    pub load_1m: f64,
    pub available_memory_bytes: u64,
    pub active_execution_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MountHealth {
    Healthy {
        filesystem_id: String,
        probe_id: String,
    },
    Unhealthy {
        reason: MountFailureReason,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MountFailureReason {
    Missing,
    LocalFallback,
    WrongFilesystem,
    ProbeNotVisible,
    ReadOnly,
    OwnershipMismatch,
    IoError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerHeartbeat {
    pub authority: RequestAuthority,
    pub resources: ResourceSnapshot,
    pub mount: MountHealth,
    pub jobs: Vec<JobSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoordinatorLease {
    pub accepted_protocol_version: u16,
    pub heartbeat_interval_seconds: u32,
    pub lease_expires_at: DateTime<Utc>,
    pub draining: bool,
    pub probe: MountProbe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MountProbe {
    pub id: String,
    pub relative_path: String,
    pub expected_contents_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionDispatch {
    pub authority: RequestAuthority,
    pub execution_id: Uuid,
    pub workspace_id: Uuid,
    pub session_id: Uuid,
    pub workspace_path: String,
    pub working_directory: String,
    pub executor_profile: String,
    pub action: Value,
    pub environment: BTreeMap<String, String>,
    pub run_reason: String,
    pub timeout_seconds: Option<u64>,
    pub persistence: PersistencePolicy,
    pub request_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PersistencePolicy {
    Ordinary,
    Persistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchAccepted {
    pub execution_id: Uuid,
    pub worker_job_id: Uuid,
    pub request_digest: String,
    pub state: JobState,
    pub last_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobSummary {
    pub execution_id: Uuid,
    pub worker_job_id: Uuid,
    pub workspace_id: Uuid,
    pub request_digest: String,
    pub state: JobState,
    pub last_sequence: u64,
    pub terminal: Option<TerminalEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Accepted,
    Starting,
    Running,
    Cancelling,
    Completed,
    Failed,
    Killed,
    Interrupted,
    Indeterminate,
    Quarantined,
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Failed
                | Self::Killed
                | Self::Interrupted
                | Self::Indeterminate
                | Self::Quarantined
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionEvent {
    pub execution_id: Uuid,
    pub sequence: u64,
    pub worker_timestamp: DateTime<Utc>,
    pub payload: ExecutionEventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEventPayload {
    Accepted,
    Starting,
    Stdout { data_base64: String },
    Stderr { data_base64: String },
    Structured { json: String },
    InteractionRequested(InteractionRequest),
    InteractionAcknowledged { interaction_id: Uuid },
    Preview(PreviewTarget),
    Completed(TerminalEvidence),
    Failed(TerminalEvidence),
    Killed(TerminalEvidence),
    Indeterminate { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventBatch {
    pub execution_id: Uuid,
    pub requested_after: u64,
    pub earliest_available: u64,
    pub latest_available: u64,
    pub replay_gap: bool,
    pub events: Vec<ExecutionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventAcknowledgement {
    pub authority: RequestAuthority,
    pub execution_id: Uuid,
    pub highest_contiguous_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancellationRequest {
    pub authority: RequestAuthority,
    pub execution_id: Uuid,
    pub graceful_timeout_seconds: u32,
    pub terminate_timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineRequest {
    pub authority: RequestAuthority,
    pub execution_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancellationStatus {
    pub execution_id: Uuid,
    pub phase: CancellationPhase,
    pub terminal: Option<TerminalEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CancellationPhase {
    Requested,
    Graceful,
    TerminatingProcessGroup,
    KillingProcessGroup,
    Confirmed,
    AlreadyTerminal,
    Indeterminate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalEvidence {
    pub state: TerminalState,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Completed,
    Failed,
    Killed,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionRequest {
    pub interaction_id: Uuid,
    pub kind: String,
    pub prompt: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub disconnect_policy: DisconnectPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionResponse {
    pub authority: RequestAuthority,
    pub execution_id: Uuid,
    pub interaction_id: Uuid,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisconnectPolicy {
    FailClosed,
    Pause,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PreviewTarget {
    pub workspace_id: Uuid,
    pub worker_job_id: Uuid,
    pub port: u16,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority() -> RequestAuthority {
        RequestAuthority {
            protocol_version: PROTOCOL_VERSION,
            coordinator_id: Uuid::nil(),
            worker_node_id: Uuid::from_u128(1),
            correlation_id: Uuid::from_u128(2),
            issued_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            nonce: "one-use".into(),
        }
    }

    #[test]
    fn event_payload_round_trips_with_stable_tag() {
        let event = ExecutionEvent {
            execution_id: Uuid::from_u128(3),
            sequence: 7,
            worker_timestamp: DateTime::from_timestamp(1_700_000_001, 0).unwrap(),
            payload: ExecutionEventPayload::Stderr {
                data_base64: "ZXJyb3I=".into(),
            },
        };

        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["payload"]["type"], "stderr");
        assert_eq!(
            serde_json::from_value::<ExecutionEvent>(json).unwrap(),
            event
        );
    }

    #[test]
    fn dispatch_round_trip_preserves_authority_and_digest() {
        let dispatch = ExecutionDispatch {
            authority: authority(),
            execution_id: Uuid::from_u128(4),
            workspace_id: Uuid::from_u128(5),
            session_id: Uuid::from_u128(6),
            workspace_path: "/srv/vibe-kanban-shared/workspaces/w".into(),
            working_directory: "/srv/vibe-kanban-shared/workspaces/w/repo".into(),
            executor_profile: "codex".into(),
            action: serde_json::json!({"type": "coding_agent"}),
            environment: BTreeMap::from([("SAFE".into(), "value".into())]),
            run_reason: "coding_agent".into(),
            timeout_seconds: Some(60),
            persistence: PersistencePolicy::Ordinary,
            request_digest: "sha256:abc".into(),
        };

        let encoded = serde_json::to_vec(&dispatch).unwrap();
        let decoded: ExecutionDispatch = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, dispatch);
    }

    #[test]
    fn only_evidence_backed_states_are_terminal() {
        assert!(!JobState::Running.is_terminal());
        assert!(!JobState::Cancelling.is_terminal());
        assert!(JobState::Completed.is_terminal());
        assert!(JobState::Indeterminate.is_terminal());
    }
}
