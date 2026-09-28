use chrono::{DateTime, Utc};
use db::models::worker_node::{WorkerMountStatus, WorkerNode, WorkerNodeStatus};
use thiserror::Error;
use uuid::Uuid;

use super::{ClusterConfig, SchedulingWeights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IneligibleReason {
    NotOnline,
    UnhealthyMount,
    MissingOrExpiredLease,
    MissingExecutor,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchedulingError {
    #[error("no eligible worker supports executor profile {executor_profile:?}")]
    NoEligibleWorkers { executor_profile: String },
    #[error("requested worker {worker_node_id} was not found")]
    RequestedWorkerNotFound { worker_node_id: Uuid },
    #[error("requested worker {worker_node_id} is ineligible: {reason:?}")]
    RequestedWorkerIneligible {
        worker_node_id: Uuid,
        reason: IneligibleReason,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct WorkerScheduler {
    weights: SchedulingWeights,
}

impl WorkerScheduler {
    pub fn new(config: &ClusterConfig) -> Self {
        Self {
            weights: config.scheduling_weights,
        }
    }

    pub fn with_weights(weights: SchedulingWeights) -> Self {
        Self { weights }
    }

    pub fn select<'a>(
        &self,
        workers: &'a [WorkerNode],
        executor_profile: &str,
        requested_worker_id: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> Result<&'a WorkerNode, SchedulingError> {
        if let Some(worker_node_id) = requested_worker_id {
            let worker = workers
                .iter()
                .find(|worker| worker.id == worker_node_id)
                .ok_or(SchedulingError::RequestedWorkerNotFound { worker_node_id })?;

            return match eligibility(worker, executor_profile, now) {
                Ok(()) => Ok(worker),
                Err(reason) => Err(SchedulingError::RequestedWorkerIneligible {
                    worker_node_id,
                    reason,
                }),
            };
        }

        workers
            .iter()
            .filter(|worker| eligibility(worker, executor_profile, now).is_ok())
            .min_by(|left, right| {
                self.score(left)
                    .total_cmp(&self.score(right))
                    .then_with(|| left.id.cmp(&right.id))
            })
            .ok_or_else(|| SchedulingError::NoEligibleWorkers {
                executor_profile: executor_profile.to_owned(),
            })
    }

    fn score(&self, worker: &WorkerNode) -> f64 {
        let load = metric(worker, "load_1m");
        let active_executions = metric(worker, "active_execution_count");

        match (load, active_executions) {
            (Some(load), Some(active_executions)) => {
                self.weights.load * load + self.weights.active_executions * active_executions
            }
            _ => f64::INFINITY,
        }
    }
}

pub fn eligibility(
    worker: &WorkerNode,
    executor_profile: &str,
    now: DateTime<Utc>,
) -> Result<(), IneligibleReason> {
    if worker.status != WorkerNodeStatus::Online {
        return Err(IneligibleReason::NotOnline);
    }
    if worker.mount_status != WorkerMountStatus::Healthy {
        return Err(IneligibleReason::UnhealthyMount);
    }
    if worker
        .lease_expires_at
        .is_none_or(|lease_expires_at| lease_expires_at <= now)
    {
        return Err(IneligibleReason::MissingOrExpiredLease);
    }
    if !worker
        .capabilities
        .get("executor_profiles")
        .and_then(|profiles| profiles.as_array())
        .is_some_and(|profiles| {
            profiles
                .iter()
                .filter_map(|profile| profile.as_str())
                .any(|advertised| advertises_executor_profile(advertised, executor_profile))
        })
    {
        return Err(IneligibleReason::MissingExecutor);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerUnavailableReason {
    NotRegistered,
    Offline,
    LeaseExpired,
}

/// The assigned worker of a sticky workspace cannot take a dispatch. The
/// message is shown to the user, so it names the host and says what to do.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("{}", self.message())]
pub struct WorkerUnavailable {
    pub worker: String,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub reason: WorkerUnavailableReason,
}

impl WorkerUnavailable {
    fn message(&self) -> String {
        let last_seen = self
            .last_heartbeat_at
            .map(|at| at.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "never".into());
        let state = match self.reason {
            WorkerUnavailableReason::NotRegistered => {
                format!(
                    "Execution server {} is not registered with this coordinator.",
                    self.worker
                )
            }
            WorkerUnavailableReason::Offline => format!(
                "Execution server {} is offline (last heartbeat {last_seen}).",
                self.worker
            ),
            WorkerUnavailableReason::LeaseExpired => format!(
                "Execution server {} has stopped responding (last heartbeat {last_seen}).",
                self.worker
            ),
        };
        format!(
            "{state} Move this workspace to another execution server, or retry once {} recovers.",
            self.worker
        )
    }
}

/// Liveness gate for dispatching to a workspace's *assigned* worker.
///
/// Deliberately narrower than [`eligibility`]: affinity is sticky, so mount
/// health and executor capability stay placement rules. What this refuses is
/// a worker the registry has already written off. Dispatching there only
/// waits out transport timeouts and then reports an opaque error.
/// A draining worker with a live lease is still dispatched to, as before.
pub fn dispatch_liveness(
    worker_node_id: Uuid,
    worker: Option<&WorkerNode>,
    now: DateTime<Utc>,
) -> Result<(), WorkerUnavailable> {
    let Some(worker) = worker else {
        return Err(WorkerUnavailable {
            worker: worker_node_id.to_string(),
            last_heartbeat_at: None,
            reason: WorkerUnavailableReason::NotRegistered,
        });
    };
    let reason = if worker.status == WorkerNodeStatus::Offline {
        WorkerUnavailableReason::Offline
    } else if worker
        .lease_expires_at
        .is_none_or(|lease_expires_at| lease_expires_at <= now)
    {
        WorkerUnavailableReason::LeaseExpired
    } else {
        return Ok(());
    };
    Err(WorkerUnavailable {
        worker: worker.hostname.clone(),
        last_heartbeat_at: worker.last_heartbeat_at,
        reason,
    })
}

/// Whether a worker advertising `advertised` can run `requested`.
///
/// `ExecutorProfileId` renders as `EXECUTOR:VARIANT` whenever a variant is set,
/// and the UI always sends one ("DEFAULT"), so a requested profile is almost
/// always qualified. A variant selects a mode of the same agent — the type's own
/// examples are "PLAN" and "ROUTER" — so a worker that can run the executor can
/// run every variant of it. Capability is therefore an executor-level property,
/// and an operator advertising the bare executor name means "any variant".
///
/// Matching on the full string alone made that configuration unschedulable:
/// workers advertising ["CLAUDE_CODE"] were rejected as MissingExecutor for
/// every request the UI produced, so a cluster could register healthy workers
/// that silently never received work.
///
/// A qualified advertisement still pins exactly one variant, so an operator who
/// wants that can keep expressing it.
fn advertises_executor_profile(advertised: &str, requested: &str) -> bool {
    if advertised == requested {
        return true;
    }
    // Bare executor advertisement matches any variant of that executor.
    !advertised.contains(':')
        && requested
            .split_once(':')
            .is_some_and(|(executor, _variant)| executor == advertised)
}

fn metric(worker: &WorkerNode, name: &str) -> Option<f64> {
    worker
        .resource_snapshot
        .get(name)
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value >= 0.0)
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::{Value, json};
    use sqlx::types::Json;

    use super::*;

    fn worker(id: u128, now: DateTime<Utc>) -> WorkerNode {
        WorkerNode {
            id: Uuid::from_u128(id),
            hostname: format!("think{id}"),
            status: WorkerNodeStatus::Online,
            worker_version: "1".into(),
            vibe_version: "1".into(),
            capabilities: Json(json!({"executor_profiles": ["codex", "claude"]})),
            resource_snapshot: Json(json!({
                "load_1m": 1.0,
                "active_execution_count": 0
            })),
            labels: Json(Value::Object(Default::default())),
            mount_status: WorkerMountStatus::Healthy,
            mount_message: None,
            last_heartbeat_at: Some(now),
            lease_expires_at: Some(now + Duration::seconds(30)),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn eligibility_rejects_each_unschedulable_state() {
        let now = Utc::now();

        let mut candidate = worker(1, now);
        candidate.status = WorkerNodeStatus::Draining;
        assert_eq!(
            eligibility(&candidate, "codex", now),
            Err(IneligibleReason::NotOnline)
        );

        candidate = worker(1, now);
        candidate.mount_status = WorkerMountStatus::ReadOnly;
        assert_eq!(
            eligibility(&candidate, "codex", now),
            Err(IneligibleReason::UnhealthyMount)
        );

        candidate = worker(1, now);
        candidate.lease_expires_at = Some(now);
        assert_eq!(
            eligibility(&candidate, "codex", now),
            Err(IneligibleReason::MissingOrExpiredLease)
        );

        candidate = worker(1, now);
        assert_eq!(
            eligibility(&candidate, "gemini", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn bare_executor_advertisement_accepts_any_variant() {
        // The UI always sends a variant, so requests arrive qualified while
        // operators configure the bare executor name. Rejecting that made
        // healthy workers permanently unschedulable in production.
        let now = Utc::now();
        let candidate = worker(1, now);

        assert_eq!(eligibility(&candidate, "codex:DEFAULT", now), Ok(()));
        assert_eq!(eligibility(&candidate, "codex:PLAN", now), Ok(()));
        assert_eq!(eligibility(&candidate, "codex", now), Ok(()));
    }

    #[test]
    fn bare_advertisement_does_not_match_a_different_executor() {
        let now = Utc::now();
        let candidate = worker(1, now);

        assert_eq!(
            eligibility(&candidate, "gemini:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
        // Prefix overlap must not be mistaken for a match.
        assert_eq!(
            eligibility(&candidate, "codexfoo:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn qualified_advertisement_still_pins_one_variant() {
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["codex:PLAN"]}));

        assert_eq!(eligibility(&candidate, "codex:PLAN", now), Ok(()));
        assert_eq!(
            eligibility(&candidate, "codex:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
        // A bare request must not widen a deliberately pinned advertisement.
        assert_eq!(
            eligibility(&candidate, "codex", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn valid_manual_selection_wins_and_invalid_selection_is_specific() {
        let now = Utc::now();
        let workers = vec![worker(1, now), worker(2, now)];
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler
                .select(&workers, "codex", Some(workers[1].id), now)
                .unwrap()
                .id,
            workers[1].id
        );
        assert_eq!(
            scheduler
                .select(&workers, "codex", Some(Uuid::from_u128(9)), now)
                .unwrap_err(),
            SchedulingError::RequestedWorkerNotFound {
                worker_node_id: Uuid::from_u128(9)
            }
        );

        let mut draining = workers;
        draining[1].status = WorkerNodeStatus::Draining;
        assert_eq!(
            scheduler
                .select(&draining, "codex", Some(draining[1].id), now)
                .unwrap_err(),
            SchedulingError::RequestedWorkerIneligible {
                worker_node_id: draining[1].id,
                reason: IneligibleReason::NotOnline,
            }
        );
    }

    #[test]
    fn weighted_score_can_prefer_load_or_execution_count() {
        let now = Utc::now();
        let mut low_load = worker(1, now);
        low_load.resource_snapshot = Json(json!({"load_1m": 0.5, "active_execution_count": 4}));
        let mut few_jobs = worker(2, now);
        few_jobs.resource_snapshot = Json(json!({"load_1m": 3.0, "active_execution_count": 0}));
        let workers = [low_load, few_jobs];

        let load_scheduler = WorkerScheduler::with_weights(SchedulingWeights {
            load: 10.0,
            active_executions: 1.0,
        });
        assert_eq!(
            load_scheduler
                .select(&workers, "codex", None, now)
                .unwrap()
                .id,
            Uuid::from_u128(1)
        );

        let execution_scheduler = WorkerScheduler::with_weights(SchedulingWeights {
            load: 1.0,
            active_executions: 10.0,
        });
        assert_eq!(
            execution_scheduler
                .select(&workers, "codex", None, now)
                .unwrap()
                .id,
            Uuid::from_u128(2)
        );
    }

    #[test]
    fn ties_are_broken_by_worker_id_regardless_of_input_order() {
        let now = Utc::now();
        let workers = [worker(2, now), worker(1, now)];
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler.select(&workers, "codex", None, now).unwrap().id,
            Uuid::from_u128(1)
        );
    }

    #[test]
    fn dispatch_liveness_admits_live_online_and_draining_workers() {
        let now = Utc::now();
        let online = worker(5, now);
        assert_eq!(dispatch_liveness(online.id, Some(&online), now), Ok(()));

        // Drain is an admission policy the worker owns; a live lease keeps
        // the pre-existing dispatch behaviour.
        let mut draining = worker(5, now);
        draining.status = WorkerNodeStatus::Draining;
        assert_eq!(dispatch_liveness(draining.id, Some(&draining), now), Ok(()));
    }

    #[test]
    fn dispatch_liveness_refuses_offline_worker_with_actionable_message() {
        let now = Utc::now();
        let heartbeat = DateTime::parse_from_rfc3339("2026-09-28T17:06:35.807Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut offline = worker(5, now);
        offline.status = WorkerNodeStatus::Offline;
        offline.last_heartbeat_at = Some(heartbeat);
        offline.lease_expires_at = Some(heartbeat + Duration::seconds(30));

        let error = dispatch_liveness(offline.id, Some(&offline), now).unwrap_err();
        assert_eq!(error.reason, WorkerUnavailableReason::Offline);
        assert_eq!(
            error.to_string(),
            "Execution server think5 is offline (last heartbeat 2026-09-28 17:06:35 UTC). \
             Move this workspace to another execution server, or retry once think5 recovers."
        );
    }

    #[test]
    fn dispatch_liveness_refuses_expired_or_missing_lease_before_expiry_sweep() {
        // `expire_leases` may not have run yet, so a row can still say
        // `online` after its lease lapsed. The lease is the authority.
        let now = Utc::now();
        let mut expired = worker(5, now);
        expired.lease_expires_at = Some(now);
        let error = dispatch_liveness(expired.id, Some(&expired), now).unwrap_err();
        assert_eq!(error.reason, WorkerUnavailableReason::LeaseExpired);
        assert!(error.to_string().contains("think5 has stopped responding"));

        let mut missing = worker(5, now);
        missing.lease_expires_at = None;
        missing.last_heartbeat_at = None;
        let error = dispatch_liveness(missing.id, Some(&missing), now).unwrap_err();
        assert_eq!(error.reason, WorkerUnavailableReason::LeaseExpired);
        assert!(error.to_string().contains("(last heartbeat never)"));

        // Expired leases on a draining row are not exempt.
        let mut draining = worker(5, now);
        draining.status = WorkerNodeStatus::Draining;
        draining.lease_expires_at = Some(now - Duration::seconds(1));
        assert_eq!(
            dispatch_liveness(draining.id, Some(&draining), now)
                .unwrap_err()
                .reason,
            WorkerUnavailableReason::LeaseExpired
        );
    }

    #[test]
    fn dispatch_liveness_refuses_unregistered_worker_by_id() {
        let id = Uuid::from_u128(9);
        let error = dispatch_liveness(id, None, Utc::now()).unwrap_err();
        assert_eq!(error.reason, WorkerUnavailableReason::NotRegistered);
        assert!(
            error
                .to_string()
                .starts_with(&format!("Execution server {id} is not registered"))
        );
    }

    #[test]
    fn no_eligible_worker_is_reported() {
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.status = WorkerNodeStatus::Offline;
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler
                .select(&[candidate], "codex", None, now)
                .unwrap_err(),
            SchedulingError::NoEligibleWorkers {
                executor_profile: "codex".into()
            }
        );
    }
}
