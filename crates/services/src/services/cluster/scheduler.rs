use chrono::{DateTime, Utc};
use db::models::worker_node::{WorkerMountStatus, WorkerNode, WorkerNodeStatus};
use executors::profile::canonical_profile_parts;
use thiserror::Error;
use uuid::Uuid;

use super::{ClusterConfig, SchedulingWeights};

fn tally(reasons: &mut Vec<(IneligibleReason, usize)>, reason: IneligibleReason) {
    match reasons.iter_mut().find(|(seen, _)| *seen == reason) {
        Some((_, count)) => *count += 1,
        None => reasons.push((reason, 1)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IneligibleReason {
    NotOnline,
    UnhealthyMount,
    MissingOrExpiredLease,
    MissingExecutor,
}

impl IneligibleReason {
    /// Operator-facing description, for messages that tally why workers were
    /// rejected. `{:?}` leaks a Rust identifier into the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::NotOnline => "not online",
            Self::UnhealthyMount => "unhealthy shared mount",
            Self::MissingOrExpiredLease => "missing or expired lease",
            Self::MissingExecutor => "does not support this executor",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchedulingError {
    /// Workers are healthy, but none advertises the requested executor.
    /// Distinct from `NoHealthyWorkers` because the remedies are opposite:
    /// change agent or advertise it, versus go repair a node.
    #[error(
        "no worker supports executor profile {executor_profile:?}; \
         available on this cluster: {}",
        if supported.is_empty() { "none".to_owned() } else { supported.join(", ") }
    )]
    ExecutorUnsupported {
        executor_profile: String,
        supported: Vec<String>,
    },
    #[error(
        "no worker is currently available ({total} registered: {})",
        format_reason_tally(reasons)
    )]
    NoHealthyWorkers {
        total: usize,
        reasons: Vec<(IneligibleReason, usize)>,
    },
    #[error("requested worker {worker_node_id} was not found")]
    RequestedWorkerNotFound { worker_node_id: Uuid },
    #[error(
        "requested worker {worker_node_id} does not support executor profile \
         {executor_profile:?}; it advertises: {}",
        if supported.is_empty() { "nothing".to_owned() } else { supported.join(", ") }
    )]
    RequestedWorkerMissingExecutor {
        worker_node_id: Uuid,
        executor_profile: String,
        supported: Vec<String>,
    },
    #[error("requested worker {worker_node_id} is ineligible: {}", reason.label())]
    RequestedWorkerIneligible {
        worker_node_id: Uuid,
        reason: IneligibleReason,
    },
}

fn format_reason_tally(reasons: &[(IneligibleReason, usize)]) -> String {
    if reasons.is_empty() {
        return "none registered".to_owned();
    }
    reasons
        .iter()
        .map(|(reason, count)| format!("{count} {}", reason.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Profiles a worker advertises, as published — never widened or reduced.
fn advertised_profiles(worker: &WorkerNode) -> Vec<String> {
    worker
        .capabilities
        .get("executor_profiles")
        .and_then(|profiles| profiles.as_array())
        .map(|profiles| {
            profiles
                .iter()
                .filter_map(|profile| profile.as_str())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
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
                // A pinned worker that is healthy but lacks the agent is a
                // capability failure like any other, so it names what the
                // worker can actually run instead of a bare reason code.
                Err(IneligibleReason::MissingExecutor) => {
                    Err(SchedulingError::RequestedWorkerMissingExecutor {
                        worker_node_id,
                        executor_profile: executor_profile.to_owned(),
                        supported: advertised_profiles(worker),
                    })
                }
                Err(reason) => Err(SchedulingError::RequestedWorkerIneligible {
                    worker_node_id,
                    reason,
                }),
            };
        }

        if let Some(selected) = workers
            .iter()
            .filter(|worker| eligibility(worker, executor_profile, now).is_ok())
            .min_by(|left, right| {
                self.score(left)
                    .total_cmp(&self.score(right))
                    .then_with(|| left.id.cmp(&right.id))
            })
        {
            return Ok(selected);
        }

        Err(Self::explain_empty_selection(
            workers,
            executor_profile,
            now,
        ))
    }

    /// Why nothing could be selected, in the terms the operator needs.
    ///
    /// "No worker supports Codex" and "every worker is down" are both reported
    /// as an empty candidate set, but they call for opposite actions, so they
    /// must not share a message. A worker rejected *only* for the executor is
    /// otherwise healthy, which makes the executor the actionable fact.
    fn explain_empty_selection(
        workers: &[WorkerNode],
        executor_profile: &str,
        now: DateTime<Utc>,
    ) -> SchedulingError {
        let mut healthy_but_unsupported = Vec::new();
        let mut reasons: Vec<(IneligibleReason, usize)> = Vec::new();

        for worker in workers {
            match eligibility(worker, executor_profile, now) {
                // Unreachable: the caller already found no candidate.
                Ok(()) => continue,
                Err(IneligibleReason::MissingExecutor) => {
                    healthy_but_unsupported.push(worker);
                    tally(&mut reasons, IneligibleReason::MissingExecutor);
                }
                Err(reason) => tally(&mut reasons, reason),
            }
        }

        if healthy_but_unsupported.is_empty() {
            return SchedulingError::NoHealthyWorkers {
                total: workers.len(),
                reasons,
            };
        }

        let mut supported: Vec<String> = healthy_but_unsupported
            .into_iter()
            .flat_map(advertised_profiles)
            .collect();
        supported.sort();
        supported.dedup();

        SchedulingError::ExecutorUnsupported {
            executor_profile: executor_profile.to_owned(),
            supported,
        }
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
///
/// The two sides also come from different places: the request is generated and
/// therefore canonical, while the advertisement was typed by an operator into
/// worker configuration. Comparing them byte-for-byte made `codex` and `CODEX`
/// different capabilities. Workers now canonicalise before registering, but
/// capabilities are written *only* at registration — `WorkerHeartbeat` does not
/// carry them and the registry preserves the stored value — so a worker that
/// registered against an older build keeps its non-canonical row for its whole
/// uptime. Canonicalising here is what keeps a coordinator-first upgrade from
/// unscheduling every worker until someone restarts it.
///
/// An advertisement naming an executor this build does not know cannot be
/// canonicalised. Those fall back to the original predicate — both halves of
/// it, including the bare-prefix branch — so an unknown name keeps exactly the
/// behaviour it has today rather than silently becoming unmatchable.
fn advertises_executor_profile(advertised: &str, requested: &str) -> bool {
    match (
        canonical_profile_parts(advertised),
        canonical_profile_parts(requested),
    ) {
        (
            Some((advertised_executor, advertised_variant)),
            Some((requested_executor, requested_variant)),
        ) => {
            advertised_executor == requested_executor
                && match (advertised_variant, requested_variant) {
                    // Bare advertisement: any variant of that executor.
                    (None, _) => true,
                    (Some(advertised), Some(requested)) => advertised == requested,
                    // A pinned advertisement is not widened by a bare request.
                    (Some(_), None) => false,
                }
        }
        _ => {
            advertised == requested
                || (!advertised.contains(':')
                    && requested
                        .split_once(':')
                        .is_some_and(|(executor, _variant)| executor == advertised))
        }
    }
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
            // Canonical, as a current worker registers them. The previous
            // fixture used lowercase "codex"/"claude" and every request was a
            // bare lowercase name — neither is producible: requests are
            // ExecutorProfileId Display output (`CODEX:DEFAULT`) and "claude"
            // is not an executor name at all. Both sides being equally
            // unrealistic is what let the case-sensitivity bug pass tests.
            capabilities: Json(json!({"executor_profiles": ["CODEX", "CLAUDE_CODE"]})),
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
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::NotOnline)
        );

        candidate = worker(1, now);
        candidate.mount_status = WorkerMountStatus::ReadOnly;
        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::UnhealthyMount)
        );

        candidate = worker(1, now);
        candidate.lease_expires_at = Some(now);
        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::MissingOrExpiredLease)
        );

        candidate = worker(1, now);
        assert_eq!(
            eligibility(&candidate, "GEMINI:DEFAULT", now),
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

        assert_eq!(eligibility(&candidate, "CODEX:DEFAULT", now), Ok(()));
        assert_eq!(eligibility(&candidate, "CODEX:PLAN", now), Ok(()));
        assert_eq!(eligibility(&candidate, "CODEX", now), Ok(()));
    }

    #[test]
    fn a_legacy_lowercase_advertisement_still_matches() {
        // Capabilities are written only at registration — heartbeats do not
        // carry them — so a worker that registered against an older build keeps
        // its non-canonical row for its entire uptime. Without this, upgrading
        // the coordinator first unschedules every worker until each is
        // restarted, which is the outage this feature exists to prevent.
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["codex", "claude-code"]}));

        assert_eq!(eligibility(&candidate, "CODEX:DEFAULT", now), Ok(()));
        assert_eq!(eligibility(&candidate, "CLAUDE_CODE:DEFAULT", now), Ok(()));
    }

    #[test]
    fn a_variant_differing_only_in_case_still_matches() {
        // Variant keys are canonicalised wherever profiles are stored, so a
        // request carries "PLAN" while an operator may have typed "plan".
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["codex:plan"]}));

        assert_eq!(eligibility(&candidate, "CODEX:PLAN", now), Ok(()));
        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn an_unresolvable_advertisement_keeps_its_previous_behaviour() {
        // An executor name this build does not know cannot be canonicalised.
        // Such rows fall back to the original predicate — including its
        // bare-prefix branch — rather than silently becoming unmatchable.
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["future_agent"]}));

        assert_eq!(eligibility(&candidate, "future_agent:DEFAULT", now), Ok(()));
        assert_eq!(eligibility(&candidate, "future_agent", now), Ok(()));
        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn an_empty_variant_advertisement_is_not_widened() {
        // "CODEX:" pins a variant, even though the variant is empty. Only the
        // worker authoring its own config may collapse that to bare "CODEX";
        // a consumer widening it would grant capability its owner never
        // advertised.
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["CODEX:"]}));

        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn bare_advertisement_does_not_match_a_different_executor() {
        let now = Utc::now();
        let candidate = worker(1, now);

        assert_eq!(
            eligibility(&candidate, "GEMINI:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
        // Prefix overlap must not be mistaken for a match.
        assert_eq!(
            eligibility(&candidate, "CODEXFOO:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
    }

    #[test]
    fn qualified_advertisement_still_pins_one_variant() {
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["CODEX:PLAN"]}));

        assert_eq!(eligibility(&candidate, "CODEX:PLAN", now), Ok(()));
        assert_eq!(
            eligibility(&candidate, "CODEX:DEFAULT", now),
            Err(IneligibleReason::MissingExecutor)
        );
        // A bare request must not widen a deliberately pinned advertisement.
        assert_eq!(
            eligibility(&candidate, "CODEX", now),
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
                .select(&workers, "CODEX:DEFAULT", Some(workers[1].id), now)
                .unwrap()
                .id,
            workers[1].id
        );
        assert_eq!(
            scheduler
                .select(&workers, "CODEX:DEFAULT", Some(Uuid::from_u128(9)), now)
                .unwrap_err(),
            SchedulingError::RequestedWorkerNotFound {
                worker_node_id: Uuid::from_u128(9)
            }
        );

        let mut draining = workers;
        draining[1].status = WorkerNodeStatus::Draining;
        assert_eq!(
            scheduler
                .select(&draining, "CODEX:DEFAULT", Some(draining[1].id), now)
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
                .select(&workers, "CODEX:DEFAULT", None, now)
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
                .select(&workers, "CODEX:DEFAULT", None, now)
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
            scheduler
                .select(&workers, "CODEX:DEFAULT", None, now)
                .unwrap()
                .id,
            Uuid::from_u128(1)
        );
    }

    #[test]
    fn an_unsupported_executor_names_what_the_cluster_can_run() {
        // The reported case: healthy workers, none advertising the agent. The
        // remedy is to pick another agent or advertise this one, so the message
        // has to say what is actually available.
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["CLAUDE_CODE"]}));
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        let error = scheduler
            .select(&[candidate], "CODEX:DEFAULT", None, now)
            .unwrap_err();
        assert_eq!(
            error,
            SchedulingError::ExecutorUnsupported {
                executor_profile: "CODEX:DEFAULT".into(),
                supported: vec!["CLAUDE_CODE".into()],
            }
        );
        let rendered = error.to_string();
        assert!(rendered.contains("CODEX:DEFAULT"), "{rendered}");
        assert!(rendered.contains("CLAUDE_CODE"), "{rendered}");
    }

    #[test]
    fn unhealthy_workers_are_reported_as_unavailable_not_as_unsupported() {
        // Blaming the executor here would send the operator to change agent
        // when the real problem is that the fleet is down.
        let now = Utc::now();
        let mut offline = worker(1, now);
        offline.status = WorkerNodeStatus::Offline;
        let mut unmounted = worker(2, now);
        unmounted.mount_status = WorkerMountStatus::ReadOnly;
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        let error = scheduler
            .select(&[offline, unmounted], "CODEX:DEFAULT", None, now)
            .unwrap_err();
        assert_eq!(
            error,
            SchedulingError::NoHealthyWorkers {
                total: 2,
                reasons: vec![
                    (IneligibleReason::NotOnline, 1),
                    (IneligibleReason::UnhealthyMount, 1),
                ],
            }
        );
        let rendered = error.to_string();
        assert!(!rendered.contains("CODEX"), "{rendered}");
    }

    #[test]
    fn an_empty_cluster_is_reported_as_unavailable() {
        let now = Utc::now();
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler
                .select(&[], "CODEX:DEFAULT", None, now)
                .unwrap_err(),
            SchedulingError::NoHealthyWorkers {
                total: 0,
                reasons: vec![],
            }
        );
    }

    #[test]
    fn a_mixed_population_prefers_the_executor_explanation() {
        // One node is down and one is healthy but lacks the agent. Switching
        // agent would work right now, so that is the actionable remedy.
        let now = Utc::now();
        let mut offline = worker(1, now);
        offline.status = WorkerNodeStatus::Offline;
        let mut healthy = worker(2, now);
        healthy.capabilities = Json(json!({"executor_profiles": ["CLAUDE_CODE"]}));
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler
                .select(&[offline, healthy], "CODEX:DEFAULT", None, now)
                .unwrap_err(),
            SchedulingError::ExecutorUnsupported {
                executor_profile: "CODEX:DEFAULT".into(),
                supported: vec!["CLAUDE_CODE".into()],
            }
        );
    }

    #[test]
    fn a_pinned_worker_missing_the_executor_says_what_it_advertises() {
        // The manual-placement path previously rendered a bare `MissingExecutor`
        // debug string, naming nothing the user could act on.
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.capabilities = Json(json!({"executor_profiles": ["CLAUDE_CODE"]}));
        let id = candidate.id;
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        let error = scheduler
            .select(&[candidate], "CODEX:DEFAULT", Some(id), now)
            .unwrap_err();
        assert_eq!(
            error,
            SchedulingError::RequestedWorkerMissingExecutor {
                worker_node_id: id,
                executor_profile: "CODEX:DEFAULT".into(),
                supported: vec!["CLAUDE_CODE".into()],
            }
        );
        assert!(error.to_string().contains("CLAUDE_CODE"));
    }

    #[test]
    fn a_pinned_unhealthy_worker_still_reports_its_own_reason() {
        let now = Utc::now();
        let mut candidate = worker(1, now);
        candidate.mount_status = WorkerMountStatus::ReadOnly;
        let id = candidate.id;
        let scheduler = WorkerScheduler::with_weights(SchedulingWeights::default());

        assert_eq!(
            scheduler
                .select(&[candidate], "CODEX:DEFAULT", Some(id), now)
                .unwrap_err(),
            SchedulingError::RequestedWorkerIneligible {
                worker_node_id: id,
                reason: IneligibleReason::UnhealthyMount,
            }
        );
    }
}

#[cfg(test)]
mod matching_parity_tests {
    use super::advertises_executor_profile;

    /// The predicate as it behaved before canonicalisation was introduced.
    fn legacy(advertised: &str, requested: &str) -> bool {
        advertised == requested
            || (!advertised.contains(':')
                && requested
                    .split_once(':')
                    .is_some_and(|(executor, _variant)| executor == advertised))
    }

    #[test]
    fn only_intended_pairs_differ_from_the_legacy_predicate() {
        // Exhaustive over the shapes that actually occur, so an unintended
        // behaviour change shows up as a failure rather than as a surprise in
        // production. Anything not listed as an intended fix must agree.
        let advertised = [
            "CODEX",
            "codex",
            "Codex",
            "claude-code",
            "CLAUDE_CODE",
            "CODEX:PLAN",
            "codex:plan",
            "CODEX:",
            "CODEX:DEFAULT",
            "codexfoo",
            "future_agent",
            "future_agent:X",
            "",
            "CURSOR",
            "CURSOR_AGENT",
        ];
        let requested = [
            "CODEX",
            "CODEX:DEFAULT",
            "CODEX:PLAN",
            "CLAUDE_CODE:DEFAULT",
            "codexfoo:DEFAULT",
            "future_agent:DEFAULT",
            "future_agent",
            "CURSOR_AGENT:DEFAULT",
            "",
        ];

        // (advertised, requested) pairs that SHOULD differ, each a deliberate
        // fix: an operator-typed spelling now matching the generated request.
        let intended_fixes = [
            ("codex", "CODEX"),
            ("codex", "CODEX:DEFAULT"),
            ("codex", "CODEX:PLAN"),
            ("Codex", "CODEX"),
            ("Codex", "CODEX:DEFAULT"),
            ("Codex", "CODEX:PLAN"),
            ("claude-code", "CLAUDE_CODE:DEFAULT"),
            ("codex:plan", "CODEX:PLAN"),
            ("CURSOR", "CURSOR_AGENT:DEFAULT"),
        ];

        for a in advertised {
            for r in requested {
                let now = advertises_executor_profile(a, r);
                let before = legacy(a, r);
                let intended = intended_fixes.contains(&(a, r));
                assert_eq!(
                    now,
                    if intended { !before } else { before },
                    "({a:?}, {r:?}): legacy={before} new={now} intended_fix={intended}"
                );
            }
        }
    }
}
