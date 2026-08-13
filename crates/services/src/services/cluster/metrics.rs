//! Coordinator-side aggregation of host metrics.
//!
//! This module is the one place where the metrics feature touches the cluster's
//! own state, and every line of it is a **read**. Constitution XIX is not a
//! style rule here: `WorkerRegistry::expire_heartbeats` — which
//! `list_workers` calls, and which an earlier draft of this design copied —
//! issues `UPDATE worker_nodes SET status = 'offline'`
//! (`crates/db/src/models/worker_node.rs`). Calling it from here would make a
//! worker's lifecycle transition depend on whether an operator happens to have
//! a monitoring drawer open. Nothing below calls it; the equivalent judgement
//! is derived in memory by [`display_health`] and thrown away with the
//! response.
//!
//! Three further properties are load-bearing:
//!
//! - **Every worker row is listed, including offline ones.** They are not
//!   polled — there is no point asking a node we believe is down — but omitting
//!   them would freeze a lease-expired worker at its last `availability`,
//!   plausibly `available`, which is exactly the "healthy here, dead in
//!   Settings" inversion the drawer exists to prevent.
//! - **The collector is subscriber-gated and holds a [`Weak`].** With nobody
//!   looking, the coordinator issues zero `GET /v1/metrics` requests.
//! - **The node map lock is never held across an `await`.** Cursors are read
//!   out, the fetches happen unlocked, and the results are written back only if
//!   the entry's generation is unchanged — so a node that deregistered
//!   mid-poll is not resurrected by its own in-flight reply.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard, Weak,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use db::models::worker_node::{WorkerMountStatus, WorkerNode, WorkerNodeStatus};
use node_metrics::{
    HostSample, MetricsSampler, NodeMetricsAvailability, NodeRole, SampleBatch,
    types::SamplerConfig,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use super::{ClusterConfig, WorkerClient, WorkerClientError};

/// Namespace for the coordinator's derived node id.
///
/// Fixed for the life of the product: it is half of the input to a UUIDv5, and
/// changing it would change every coordinator's id, which is the thing the v5
/// derivation exists to keep stable.
pub const COORDINATOR_NAMESPACE: Uuid = Uuid::from_u128(0x9f3d_5c1e_7a4b_4f28_a1c6_d0e5_b83f_2740);

/// How long a successful contact with no fresh sample is tolerated before the
/// node is reported [`NodeMetricsAvailability::Stale`] rather than available.
/// Five ticks, so ordinary jitter does not flap the badge.
const STALE_AFTER_TICKS: u32 = 5;

/// The cluster's judgement of a node, as opposed to whether we could read its
/// metrics.
///
/// The distinction is the point: `availability` says whether a `/proc` read
/// reached us, `health` says what the scheduler thinks. The drawer has to carry
/// both or it cannot be checked against Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct NodeHealth {
    /// `worker_nodes.status`, adjusted **in memory** for a lapsed lease.
    pub status: WorkerNodeStatus,
    /// `None` for the coordinator, which has no worker row.
    pub mount_status: Option<WorkerMountStatus>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    /// `status == online && mount_status == healthy`, matching the derivation
    /// the workers settings section already renders.
    pub schedulable: bool,
}

/// One node's entry in the cluster snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MetricsNode {
    pub node_id: Uuid,
    /// From the host's own sample where one exists, falling back to
    /// `worker_nodes.hostname` — the sample is authoritative because it was
    /// read on the machine being described.
    pub hostname: String,
    pub role: NodeRole,
    /// `None` for the coordinator, which has no worker row to judge it by.
    pub health: Option<NodeHealth>,
    pub availability: NodeMetricsAvailability,
    /// The only sample carrying a process table.
    pub latest: Option<HostSample>,
    /// Bounded; `processes` is `None` on every entry.
    pub history: Vec<HostSample>,
    pub last_contact_at: Option<DateTime<Utc>>,
}

/// The whole cluster at one instant.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ClusterMetricsSnapshot {
    /// **A map, not a `Vec`.** Node-keyed addressing is what lets a worker
    /// register or deregister mid-stream without shifting an index and landing
    /// a `replace` on the wrong row.
    pub nodes: BTreeMap<Uuid, MetricsNode>,
    pub generated_at: DateTime<Utc>,
    /// Served rather than hardcoded on the client, so the sparkline x-axis
    /// stays correct if the cadence ever changes.
    pub sample_interval_ms: u64,
    pub disk_alert_thresholds: node_metrics::types::DiskAlertThresholds,
}

#[derive(Debug, Error)]
pub enum ClusterMetricsError {
    #[error("worker rows could not be read: {0}")]
    Database(#[from] sqlx::Error),
}

/// Why one node's poll did not produce samples.
///
/// Deliberately only two shapes: a version skew and everything else. The
/// distinction matters because an old worker is not a broken one, and rendering
/// it as unreachable would send an operator looking for a network fault that
/// does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricsFetchError {
    Unreachable(String),
    NotImplemented,
}

/// The coordinator's channel to one worker's metrics.
///
/// A trait rather than a concrete [`WorkerClient`] so the aggregation rules —
/// per-node failure isolation, generation-conditional write-back, the
/// subscriber gate — are testable without a socket.
#[async_trait]
pub trait WorkerMetricsSource: Send + Sync + 'static {
    async fn fetch(
        &self,
        worker_node_id: Uuid,
        after: u64,
    ) -> Result<SampleBatch, MetricsFetchError>;
}

#[async_trait]
impl WorkerMetricsSource for WorkerClient {
    async fn fetch(
        &self,
        worker_node_id: Uuid,
        after: u64,
    ) -> Result<SampleBatch, MetricsFetchError> {
        self.metrics(worker_node_id, after)
            .await
            .map_err(classify_fetch_error)
    }
}

/// Which of the two failure shapes a transport error is.
///
/// A free function rather than an inline `match` so the 404-is-a-version-skew
/// rule is testable without a socket: the only other way to reach it is to
/// stand up a worker that answers `404`.
fn classify_fetch_error(error: WorkerClientError) -> MetricsFetchError {
    match error {
        WorkerClientError::NotImplemented { .. } => MetricsFetchError::NotImplemented,
        WorkerClientError::Rejected { status, .. } if status == reqwest::StatusCode::NOT_FOUND => {
            MetricsFetchError::NotImplemented
        }
        error => MetricsFetchError::Unreachable(error.to_string()),
    }
}

/// Everything retained per node. Bounded by construction: the ring is capped at
/// `retention` entries and only the newest carries a process table, so the
/// footprint is a function of the node count, never of uptime.
#[derive(Debug)]
struct NodeState {
    /// Bumped whenever the entry is (re)created. A reply that arrives after its
    /// entry was replaced carries a stale generation and is dropped.
    generation: u64,
    cursor: u64,
    availability: NodeMetricsAvailability,
    hostname: Option<String>,
    latest: Option<HostSample>,
    history: VecDeque<HostSample>,
    last_contact_at: Option<DateTime<Utc>>,
    last_sample_at: Option<DateTime<Utc>>,
}

impl NodeState {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            cursor: 0,
            availability: NodeMetricsAvailability::NotCollected,
            hostname: None,
            latest: None,
            history: VecDeque::new(),
            last_contact_at: None,
            last_sample_at: None,
        }
    }

    fn clear_readings(&mut self) {
        self.latest = None;
        self.history.clear();
    }
}

pub struct ClusterMetricsService {
    pool: SqlitePool,
    source: Option<Arc<dyn WorkerMetricsSource>>,
    local: Arc<MetricsSampler>,
    sampler_config: SamplerConfig,
    disk_alert_thresholds: node_metrics::types::DiskAlertThresholds,
    coordinator_node_id: Uuid,
    coordinator_hostname: String,
    nodes: Mutex<HashMap<Uuid, NodeState>>,
    generations: AtomicU64,
    subscribers: AtomicUsize,
    collector_running: AtomicBool,
    /// Serialises collection rounds so a burst of REST reads cannot stampede
    /// every worker at once. A round that finds it held simply skips.
    round: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for ClusterMetricsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterMetricsService")
            .field("coordinator_node_id", &self.coordinator_node_id)
            .field("subscribers", &self.subscribers.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl ClusterMetricsService {
    pub fn new(
        pool: SqlitePool,
        config: &ClusterConfig,
        client: Option<WorkerClient>,
    ) -> Arc<Self> {
        let source = client.map(|client| Arc::new(client) as Arc<dyn WorkerMetricsSource>);
        Self::with_source(pool, config, source, SamplerConfig::default())
    }

    pub fn with_source(
        pool: SqlitePool,
        config: &ClusterConfig,
        source: Option<Arc<dyn WorkerMetricsSource>>,
        sampler_config: SamplerConfig,
    ) -> Arc<Self> {
        let coordinator_hostname = local_hostname();
        // `coordinator_id` is `Option` and `None` unless clustering is enabled,
        // but `nodes` is keyed by `Uuid` and the panel's persisted selection
        // must survive a restart. A v5 over the hostname is stable across boots
        // in a way a `new_v4()` would not be.
        let coordinator_node_id = config.coordinator_id.unwrap_or_else(|| {
            Uuid::new_v5(&COORDINATOR_NAMESPACE, coordinator_hostname.as_bytes())
        });
        Arc::new(Self {
            pool,
            source,
            local: Arc::new(MetricsSampler::new(sampler_config.clone())),
            sampler_config,
            disk_alert_thresholds: disk_alert_thresholds_from_env(),
            coordinator_node_id,
            coordinator_hostname,
            nodes: Mutex::new(HashMap::new()),
            generations: AtomicU64::new(1),
            subscribers: AtomicUsize::new(0),
            collector_running: AtomicBool::new(false),
            round: tokio::sync::Mutex::new(()),
        })
    }

    /// Start the coordinator's own sampler.
    ///
    /// Unlike the worker collector this runs unconditionally: it is one local
    /// `/proc` read per tick with no network cost, and without it the
    /// coordinator's own card would be empty the moment a drawer opened.
    pub fn spawn_local_sampler(&self, shutdown: CancellationToken) -> tokio::task::JoinHandle<()> {
        let (tx, rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            shutdown.cancelled().await;
            let _ = tx.send(true);
        });
        MetricsSampler::spawn(&self.local, rx)
    }

    pub fn coordinator_node_id(&self) -> Uuid {
        self.coordinator_node_id
    }

    pub fn sample_interval_ms(&self) -> u64 {
        self.sampler_config.interval_ms
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.load(Ordering::SeqCst)
    }

    pub fn is_collecting(&self) -> bool {
        self.collector_running.load(Ordering::SeqCst)
    }

    /// Register a live viewer and start the collector if it was idle.
    ///
    /// The returned guard decrements on drop, which is what makes the
    /// "abnormal close still releases the slot" requirement structural rather
    /// than a branch someone has to remember to write.
    pub fn subscribe(self: &Arc<Self>) -> MetricsSubscription {
        self.subscribers.fetch_add(1, Ordering::SeqCst);
        self.ensure_collector();
        MetricsSubscription {
            service: Arc::downgrade(self),
        }
    }

    fn ensure_collector(self: &Arc<Self>) {
        if self.collector_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        let interval = self.sampler_config.interval();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                // Upgraded for the duration of one round only: between ticks
                // this task holds nothing, so it cannot keep the deployment
                // alive past shutdown.
                let Some(service) = weak.upgrade() else {
                    return;
                };
                if service.subscribers.load(Ordering::SeqCst) == 0 {
                    service.collector_running.store(false, Ordering::SeqCst);
                    // A subscriber arriving between the load and the store
                    // would have found the flag set and declined to spawn a
                    // replacement, so reclaim the slot rather than exiting.
                    if service.subscribers.load(Ordering::SeqCst) == 0
                        || service.collector_running.swap(true, Ordering::SeqCst)
                    {
                        return;
                    }
                    continue;
                }
                service.collect_round().await;
            }
        });
    }

    /// One bounded collection round, for the REST fallback.
    ///
    /// Without this, a cluster whose drawer has been shut for longer than the
    /// retention window would answer `latest: null` for every worker — a
    /// fallback that falls back to nothing. It does **not** start the
    /// continuous collector.
    pub async fn collect_once_if_idle(&self) {
        if self.collector_running.load(Ordering::SeqCst) {
            return;
        }
        self.collect_round().await;
    }

    /// Poll every pollable worker once and fold the results in.
    ///
    /// Failure is isolated per node: one unreachable worker becomes its own
    /// `availability` and leaves every peer untouched.
    pub async fn collect_round(&self) {
        let Ok(_round) = self.round.try_lock() else {
            return;
        };
        let now = Utc::now();
        let workers = match WorkerNode::fetch_all(&self.pool).await {
            Ok(workers) => workers,
            Err(error) => {
                tracing::debug!(%error, "cluster metrics: worker rows unavailable this round");
                return;
            }
        };

        self.ingest_local(now);

        let targets: Vec<(Uuid, u64, u64)> = {
            let mut nodes = self.lock();
            self.reconcile(&mut nodes, &workers);
            workers
                .iter()
                // Offline nodes are listed but never polled. They still get
                // their real `health`; see the module docs.
                .filter(|worker| display_health(worker, now).status != WorkerNodeStatus::Offline)
                .filter_map(|worker| {
                    let state = nodes.get(&worker.id)?;
                    Some((worker.id, state.generation, state.cursor))
                })
                .collect()
        };

        let Some(source) = self.source.clone() else {
            self.expire(now);
            return;
        };
        if targets.is_empty() {
            self.expire(now);
            return;
        }

        // The lock is released for the whole of this: a slow node must not
        // block a reader building a snapshot.
        let results =
            futures::future::join_all(targets.into_iter().map(|(node_id, generation, cursor)| {
                let source = source.clone();
                async move {
                    let outcome = source.fetch(node_id, cursor).await;
                    (node_id, generation, cursor, outcome)
                }
            }))
            .await;

        {
            let mut nodes = self.lock();
            for (node_id, generation, cursor, outcome) in results {
                let Some(state) = nodes.get_mut(&node_id) else {
                    // Deregistered while its own request was in flight.
                    // Re-inserting it here would resurrect a node the operator
                    // has already removed.
                    continue;
                };
                if state.generation != generation {
                    continue;
                }
                apply_outcome(state, cursor, outcome, now, &self.sampler_config);
            }
        }
        self.expire(now);
    }

    /// Build the browser-facing snapshot.
    ///
    /// Reads `worker_nodes`; writes nothing, anywhere, on any branch.
    pub async fn snapshot(&self) -> Result<ClusterMetricsSnapshot, ClusterMetricsError> {
        let now = Utc::now();
        let workers = WorkerNode::fetch_all(&self.pool).await?;
        self.ingest_local(now);

        let mut nodes = BTreeMap::new();
        {
            let mut map = self.lock();
            self.reconcile(&mut map, &workers);
            expire_locked(&mut map, now, self.retention_window());

            let coordinator = map.get(&self.coordinator_node_id);
            nodes.insert(
                self.coordinator_node_id,
                MetricsNode {
                    node_id: self.coordinator_node_id,
                    hostname: coordinator
                        .and_then(|state| state.hostname.clone())
                        .unwrap_or_else(|| self.coordinator_hostname.clone()),
                    role: NodeRole::Coordinator,
                    // The coordinator has no `worker_nodes` row, and
                    // synthesising one would put it in front of
                    // `scheduler::eligibility`.
                    health: None,
                    availability: coordinator
                        .map(|state| state.availability.clone())
                        .unwrap_or(NodeMetricsAvailability::NotCollected),
                    latest: coordinator.and_then(|state| state.latest.clone()),
                    history: coordinator
                        .map(|state| state.history.iter().cloned().collect())
                        .unwrap_or_default(),
                    last_contact_at: coordinator.and_then(|state| state.last_contact_at),
                },
            );

            for worker in &workers {
                let state = map.get(&worker.id);
                nodes.insert(
                    worker.id,
                    MetricsNode {
                        node_id: worker.id,
                        hostname: state
                            .and_then(|state| state.hostname.clone())
                            .unwrap_or_else(|| worker.hostname.clone()),
                        role: NodeRole::Worker,
                        health: Some(display_health(worker, now)),
                        availability: state
                            .map(|state| state.availability.clone())
                            .unwrap_or(NodeMetricsAvailability::NotCollected),
                        latest: state.and_then(|state| state.latest.clone()),
                        history: state
                            .map(|state| state.history.iter().cloned().collect())
                            .unwrap_or_default(),
                        last_contact_at: state.and_then(|state| state.last_contact_at),
                    },
                );
            }
        }

        Ok(ClusterMetricsSnapshot {
            nodes,
            generated_at: now,
            sample_interval_ms: self.sampler_config.interval_ms,
            disk_alert_thresholds: self.disk_alert_thresholds,
        })
    }

    /// Fold the local sampler's ring into the coordinator's entry.
    ///
    /// Synchronous by design — [`MetricsSampler::since`] takes no `await`, so
    /// this never runs while the node map lock is held across a suspension
    /// point.
    fn ingest_local(&self, now: DateTime<Utc>) {
        let cursor = {
            let nodes = self.lock();
            nodes
                .get(&self.coordinator_node_id)
                .map(|state| state.cursor)
                .unwrap_or(0)
        };
        let batch = self.local.since(cursor);
        let unsupported = batch.latest_sequence == 0 && !cfg!(target_os = "linux");

        let mut nodes = self.lock();
        let generation = self.generations.fetch_add(1, Ordering::Relaxed);
        let state = nodes
            .entry(self.coordinator_node_id)
            .or_insert_with(|| NodeState::new(generation));
        if unsupported {
            state.availability = NodeMetricsAvailability::Unsupported {
                platform: std::env::consts::OS.to_owned(),
            };
            return;
        }
        apply_outcome(state, cursor, Ok(batch), now, &self.sampler_config);
    }

    /// Reconcile the in-memory node set with the worker rows.
    ///
    /// Entries for rows that no longer exist are dropped rather than left to
    /// rot; new rows get a fresh generation so a reply already in flight for a
    /// recycled id cannot land on the new entry.
    fn reconcile(&self, nodes: &mut HashMap<Uuid, NodeState>, workers: &[WorkerNode]) {
        let known: std::collections::HashSet<Uuid> =
            workers.iter().map(|worker| worker.id).collect();
        nodes.retain(|id, _| *id == self.coordinator_node_id || known.contains(id));
        for worker in workers {
            nodes.entry(worker.id).or_insert_with(|| {
                NodeState::new(self.generations.fetch_add(1, Ordering::Relaxed))
            });
        }
    }

    fn retention_window(&self) -> TimeDelta {
        TimeDelta::from_std(Duration::from_millis(
            self.sampler_config.interval_ms * u64::from(self.sampler_config.retention),
        ))
        .unwrap_or_else(|_| TimeDelta::seconds(300))
    }

    fn expire(&self, now: DateTime<Utc>) {
        let window = self.retention_window();
        expire_locked(&mut self.lock(), now, window);
    }

    /// A poisoned lock means a previous holder panicked mid-update. The map is
    /// a cache of observations, not a ledger; recovering beats propagating a
    /// panic into every reader of the drawer.
    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, NodeState>> {
        self.nodes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Node ids the service is currently tracking in memory, independent of
    /// what a snapshot would render. Tests use it to observe that a node
    /// removed mid-poll is not re-inserted by its own late reply.
    #[cfg(test)]
    fn tracked_node_ids(&self) -> Vec<Uuid> {
        let mut ids: Vec<Uuid> = self.lock().keys().copied().collect();
        ids.sort();
        ids
    }
}

/// Releases its subscriber slot on drop — clean close, abnormal close, panic,
/// or task cancellation alike.
pub struct MetricsSubscription {
    service: Weak<ClusterMetricsService>,
}

impl Drop for MetricsSubscription {
    fn drop(&mut self) {
        if let Some(service) = self.service.upgrade() {
            service.subscribers.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl std::fmt::Debug for MetricsSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsSubscription").finish()
    }
}

/// The cluster's judgement of one worker, derived for display.
///
/// This mirrors `WorkerNode::expire_leases` exactly — an *online* row whose
/// lease has lapsed reads as offline — so the drawer and Settings agree. It
/// mirrors the query rather than the looser "any expired lease reads offline"
/// because a draining node is left alone by the real expiry too, and the
/// requirement is parity with what Settings shows, not a second opinion.
///
/// Nothing here writes. That is the entire point.
pub fn display_health(worker: &WorkerNode, now: DateTime<Utc>) -> NodeHealth {
    let lease_lapsed = worker
        .lease_expires_at
        .is_none_or(|expires_at| expires_at <= now);
    let status = if worker.status == WorkerNodeStatus::Online && lease_lapsed {
        WorkerNodeStatus::Offline
    } else {
        worker.status
    };
    NodeHealth {
        status,
        mount_status: Some(worker.mount_status.clone()),
        lease_expires_at: worker.lease_expires_at,
        schedulable: status == WorkerNodeStatus::Online && worker.mount_status.is_healthy(),
    }
}

fn apply_outcome(
    state: &mut NodeState,
    cursor: u64,
    outcome: Result<SampleBatch, MetricsFetchError>,
    now: DateTime<Utc>,
    config: &SamplerConfig,
) {
    let batch = match outcome {
        Ok(batch) => batch,
        Err(MetricsFetchError::NotImplemented) => {
            state.availability = NodeMetricsAvailability::NotImplemented;
            return;
        }
        Err(MetricsFetchError::Unreachable(reason)) => {
            state.availability = NodeMetricsAvailability::Unreachable { reason };
            return;
        }
    };

    state.last_contact_at = Some(now);

    // The worker restarted: its sequences began again below our cursor, so
    // everything retained here describes a previous process.
    if batch.latest_sequence < cursor {
        state.cursor = 0;
        state.clear_readings();
    }
    // A gap is benign for metrics — a hole in a graph — but the retained
    // history is no longer contiguous, so drop it and let the stream
    // resnapshot rather than draw a straight line across missing time.
    if batch.has_gap(cursor) {
        state.clear_readings();
    }

    let received = !batch.samples.is_empty();
    for sample in batch.samples {
        state.last_sample_at = Some(sample.captured_at);
        state.hostname = Some(sample.hostname.clone());
        state.cursor = state.cursor.max(sample.sequence);
        let mut history_entry = sample.clone();
        // Only the newest entry carries a process table; the table is most of a
        // sample's size and nothing plots it over time.
        history_entry.processes = None;
        state.history.push_back(history_entry);
        state.latest = Some(sample);
    }
    state.cursor = state.cursor.max(batch.latest_sequence);

    let retention = (config.retention as usize).max(1);
    while state.history.len() > retention {
        state.history.pop_front();
    }

    let stale_after =
        TimeDelta::milliseconds((config.interval_ms * u64::from(STALE_AFTER_TICKS)) as i64);
    state.availability = if received {
        NodeMetricsAvailability::Available
    } else {
        let since = state.last_sample_at.unwrap_or(now);
        if state.latest.is_some() && now - since < stale_after {
            NodeMetricsAvailability::Available
        } else {
            NodeMetricsAvailability::Stale { since }
        }
    };
}

/// Drop readings that have aged out of the retention window on a node that is
/// no longer reporting. Five-minute-old numbers next to a live graph are not
/// evidence of anything, and leaving them there is how a dead host keeps
/// looking busy.
fn expire_locked(nodes: &mut HashMap<Uuid, NodeState>, now: DateTime<Utc>, window: TimeDelta) {
    for state in nodes.values_mut() {
        if state.availability == NodeMetricsAvailability::Available {
            continue;
        }
        let expired = state
            .latest
            .as_ref()
            .map(|sample| now - sample.captured_at > window)
            .unwrap_or(false);
        if expired {
            state.clear_readings();
        }
    }
}

/// The coordinator's hostname.
///
/// `ClusterConfig` has no hostname field, and this value feeds a UUIDv5, so it
/// has to be stable across restarts rather than merely correct.
fn local_hostname() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(name) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_owned();
            }
        }
    }
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_owned();
            if !value.is_empty() {
                return value;
            }
        }
    }
    "coordinator".to_owned()
}

fn disk_alert_thresholds_from_env() -> node_metrics::types::DiskAlertThresholds {
    use node_metrics::types::DiskAlertThresholds;

    fn parse<T: std::str::FromStr>(key: &str, fallback: T) -> T {
        std::env::var(key)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback)
    }

    let defaults = DiskAlertThresholds::default();
    let configured = DiskAlertThresholds {
        warning_free_percent: parse(
            "VK_DISK_WARNING_FREE_PERCENT",
            defaults.warning_free_percent,
        ),
        warning_free_bytes: parse("VK_DISK_WARNING_FREE_BYTES", defaults.warning_free_bytes),
        critical_free_percent: parse(
            "VK_DISK_CRITICAL_FREE_PERCENT",
            defaults.critical_free_percent,
        ),
        critical_free_bytes: parse("VK_DISK_CRITICAL_FREE_BYTES", defaults.critical_free_bytes),
    };
    if let Err(reason) = configured.validate() {
        tracing::warn!(%reason, "invalid disk alert thresholds; using defaults");
        defaults
    } else {
        configured
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use db::models::worker_node::UpsertWorkerNode;
    use node_metrics::types::{CpuSample, MemorySample};
    use sqlx::sqlite::SqlitePoolOptions;
    use tokio::sync::Notify;

    use super::*;
    use crate::services::cluster::eligibility;

    /// Fast enough that the collector's subscriber re-check happens within a
    /// test's patience, short enough that the retention window is a few hundred
    /// milliseconds rather than five minutes.
    fn test_sampler_config() -> SamplerConfig {
        SamplerConfig {
            interval_ms: 20,
            retention: 4,
            max_processes: 5,
        }
    }

    fn config(coordinator_id: Option<Uuid>) -> ClusterConfig {
        ClusterConfig {
            coordinator_id,
            ..ClusterConfig::default()
        }
    }

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        pool
    }

    /// A worker row in the `online` state, healthy mount, with the given lease.
    async fn insert_worker(
        pool: &SqlitePool,
        hostname: &str,
        lease_expires_at: DateTime<Utc>,
    ) -> WorkerNode {
        WorkerNode::upsert_heartbeat(
            pool,
            &UpsertWorkerNode {
                id: Uuid::new_v4(),
                hostname: hostname.to_owned(),
                worker_version: "1".into(),
                vibe_version: "1".into(),
                capabilities: serde_json::json!({}),
                resource_snapshot: serde_json::json!({"load_1m": 0.5, "active_execution_count": 0}),
                labels: serde_json::json!({}),
                mount_status: WorkerMountStatus::Healthy,
                mount_message: None,
                heartbeat_at: lease_expires_at - TimeDelta::seconds(30),
                lease_expires_at,
            },
        )
        .await
        .unwrap()
    }

    /// The two columns `expire_heartbeats` would move, read back as raw text so
    /// the assertion is over what is stored rather than over a re-derived value.
    async fn row_fingerprint(pool: &SqlitePool, id: Uuid) -> (String, String) {
        sqlx::query_as::<_, (String, String)>(
            "SELECT status, updated_at FROM worker_nodes WHERE id = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn sample(sequence: u64, hostname: &str, captured_at: DateTime<Utc>) -> HostSample {
        HostSample {
            sequence,
            hostname: hostname.to_owned(),
            captured_at,
            interval_ms: Some(20),
            uptime_seconds: Some(3600),
            cpu: CpuSample {
                model: None,
                core_count: Some(4),
                total_busy_percent: Some(12.5),
                per_core_busy: None,
                load_1m: None,
                load_5m: None,
                load_15m: None,
                frequency_mhz: None,
                temperature_celsius: None,
            },
            memory: MemorySample {
                total_bytes: Some(1024),
                available_bytes: Some(512),
                used_bytes: Some(512),
                cached_bytes: None,
                swap_total_bytes: None,
                swap_used_bytes: None,
            },
            filesystems: None,
            networks: None,
            processes: None,
            degraded: Vec::new(),
        }
    }

    fn batch(samples: Vec<HostSample>) -> SampleBatch {
        let earliest_retained_sequence = samples.first().map(|s| s.sequence).unwrap_or(0);
        let latest_sequence = samples.last().map(|s| s.sequence).unwrap_or(0);
        SampleBatch {
            samples,
            earliest_retained_sequence,
            latest_sequence,
        }
    }

    fn empty_batch(latest_sequence: u64) -> SampleBatch {
        SampleBatch {
            samples: Vec::new(),
            earliest_retained_sequence: latest_sequence,
            latest_sequence,
        }
    }

    /// A scripted [`WorkerMetricsSource`] that records which nodes were asked.
    struct ScriptedSource {
        outcomes: Mutex<HashMap<Uuid, Result<SampleBatch, MetricsFetchError>>>,
        polled: Mutex<Vec<Uuid>>,
        calls: AtomicUsize,
    }

    impl ScriptedSource {
        fn new(outcomes: Vec<(Uuid, Result<SampleBatch, MetricsFetchError>)>) -> Arc<Self> {
            Arc::new(Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                polled: Mutex::new(Vec::new()),
                calls: AtomicUsize::new(0),
            })
        }

        fn polled(&self) -> Vec<Uuid> {
            let mut polled = self.polled.lock().unwrap().clone();
            polled.sort();
            polled
        }
    }

    #[async_trait]
    impl WorkerMetricsSource for ScriptedSource {
        async fn fetch(
            &self,
            worker_node_id: Uuid,
            _after: u64,
        ) -> Result<SampleBatch, MetricsFetchError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.polled.lock().unwrap().push(worker_node_id);
            self.outcomes
                .lock()
                .unwrap()
                .get(&worker_node_id)
                .cloned()
                .unwrap_or(Err(MetricsFetchError::Unreachable("unscripted".into())))
        }
    }

    fn service(
        pool: &SqlitePool,
        source: Option<Arc<dyn WorkerMetricsSource>>,
    ) -> Arc<ClusterMetricsService> {
        ClusterMetricsService::with_source(
            pool.clone(),
            &config(Some(Uuid::new_v4())),
            source,
            test_sampler_config(),
        )
    }

    /// The regression test for analysis E1.
    ///
    /// An earlier draft called `WorkerRegistry::expire_heartbeats` from the
    /// listing path, which issues `UPDATE worker_nodes SET status = 'offline'`.
    /// The worker below is exactly the row that statement targets — `online`
    /// with a lapsed lease — so if any snapshot path ever calls it again, the
    /// stored `status`/`updated_at` move and this fails. The `expire_leases`
    /// call at the end proves the row really was expirable, which is what stops
    /// this from passing vacuously.
    #[tokio::test]
    async fn successful_snapshot_never_writes_worker_rows() {
        let pool = test_pool().await;
        let worker = insert_worker(&pool, "think4", Utc::now() - TimeDelta::seconds(60)).await;
        assert_eq!(worker.status, WorkerNodeStatus::Online);
        let before = row_fingerprint(&pool, worker.id).await;

        let service = service(&pool, None);
        let snapshot = service.snapshot().await.unwrap();
        // The judgement is made — in memory only.
        assert_eq!(
            snapshot.nodes[&worker.id].health.as_ref().unwrap().status,
            WorkerNodeStatus::Offline
        );
        assert_eq!(row_fingerprint(&pool, worker.id).await, before);

        // A second read, and a collection round, are just as read-only.
        service.snapshot().await.unwrap();
        service.collect_round().await;
        assert_eq!(row_fingerprint(&pool, worker.id).await, before);

        // Control: the row was genuinely a candidate for expiry, so the
        // assertions above are not passing because there was nothing to write.
        assert_eq!(
            WorkerNode::expire_leases(&pool, Utc::now()).await.unwrap(),
            1
        );
        assert_ne!(row_fingerprint(&pool, worker.id).await, before);
    }

    #[tokio::test]
    async fn one_failing_node_does_not_disturb_its_peers() {
        let pool = test_pool().await;
        let lease = Utc::now() + TimeDelta::seconds(60);
        let healthy = insert_worker(&pool, "think3", lease).await;
        let broken = insert_worker(&pool, "think4", lease).await;
        let now = Utc::now();

        let source = ScriptedSource::new(vec![
            (healthy.id, Ok(batch(vec![sample(1, "think3", now)]))),
            (
                broken.id,
                Err(MetricsFetchError::Unreachable("connection refused".into())),
            ),
        ]);
        let service = service(&pool, Some(source.clone()));
        service.collect_round().await;

        let snapshot = service.snapshot().await.unwrap();
        assert_eq!(
            snapshot.nodes[&healthy.id].availability,
            NodeMetricsAvailability::Available
        );
        assert_eq!(snapshot.nodes[&healthy.id].hostname, "think3");
        assert!(snapshot.nodes[&healthy.id].latest.is_some());
        assert_eq!(
            snapshot.nodes[&broken.id].availability,
            NodeMetricsAvailability::Unreachable {
                reason: "connection refused".into()
            }
        );
        assert!(snapshot.nodes[&broken.id].latest.is_none());
    }

    #[tokio::test]
    async fn an_old_worker_reports_not_implemented_rather_than_unreachable() {
        let pool = test_pool().await;
        let worker = insert_worker(&pool, "think4", Utc::now() + TimeDelta::seconds(60)).await;
        let source = ScriptedSource::new(vec![(worker.id, Err(MetricsFetchError::NotImplemented))]);
        let service = service(&pool, Some(source.clone()));
        service.collect_round().await;

        assert_eq!(
            service.snapshot().await.unwrap().nodes[&worker.id].availability,
            NodeMetricsAvailability::NotImplemented
        );
    }

    /// The 404 → `NotImplemented` mapping itself, which is where a version skew
    /// gets told apart from a fault.
    #[test]
    fn a_404_is_a_version_skew_not_a_fault() {
        assert_eq!(
            classify_fetch_error(WorkerClientError::Rejected {
                status: reqwest::StatusCode::NOT_FOUND,
                message: "Not Found".into(),
            }),
            MetricsFetchError::NotImplemented
        );
        assert_eq!(
            classify_fetch_error(WorkerClientError::NotImplemented {
                path: "/v1/metrics".into(),
            }),
            MetricsFetchError::NotImplemented
        );
        assert!(matches!(
            classify_fetch_error(WorkerClientError::Rejected {
                status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                message: "boom".into(),
            }),
            MetricsFetchError::Unreachable(_)
        ));
        assert!(matches!(
            classify_fetch_error(WorkerClientError::EndpointNotFound(Uuid::nil())),
            MetricsFetchError::Unreachable(_)
        ));
    }

    /// FR-21 / Constitution XIX on the failure path: a node we could not reach
    /// is still whatever the cluster's own evidence channel says it is.
    #[tokio::test]
    async fn a_metrics_failure_changes_no_scheduling_state() {
        let pool = test_pool().await;
        let now = Utc::now();
        let worker = insert_worker(&pool, "think4", now + TimeDelta::seconds(60)).await;
        let before = WorkerNode::find_by_id(&pool, worker.id)
            .await
            .unwrap()
            .unwrap();
        let before_eligibility = format!("{:?}", eligibility(&before, "codex", now));
        let before_row = row_fingerprint(&pool, worker.id).await;

        let source = ScriptedSource::new(vec![(
            worker.id,
            Err(MetricsFetchError::Unreachable("timed out".into())),
        )]);
        let service = service(&pool, Some(source));
        service.collect_round().await;
        service.snapshot().await.unwrap();

        let after = WorkerNode::find_by_id(&pool, worker.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.status, before.status);
        assert_eq!(after.lease_expires_at, before.lease_expires_at);
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(row_fingerprint(&pool, worker.id).await, before_row);
        assert_eq!(
            format!("{:?}", eligibility(&after, "codex", now)),
            before_eligibility
        );
    }

    /// Analysis E6: an offline worker is listed but not polled. Omitting it
    /// would leave the drawer showing whatever it last saw — plausibly
    /// `available` — for a node Settings calls dead.
    #[tokio::test]
    async fn a_lease_expired_worker_is_listed_offline_and_never_polled() {
        let pool = test_pool().await;
        let now = Utc::now();
        let live = insert_worker(&pool, "think3", now + TimeDelta::seconds(60)).await;
        let lapsed = insert_worker(&pool, "think4", now - TimeDelta::seconds(60)).await;

        let source = ScriptedSource::new(vec![
            (live.id, Ok(batch(vec![sample(1, "think3", now)]))),
            (lapsed.id, Ok(batch(vec![sample(1, "think4", now)]))),
        ]);
        let service = service(&pool, Some(source.clone()));
        service.collect_round().await;

        assert_eq!(source.polled(), vec![live.id]);

        let snapshot = service.snapshot().await.unwrap();
        let node = &snapshot.nodes[&lapsed.id];
        let health = node.health.as_ref().unwrap();
        assert_eq!(health.status, WorkerNodeStatus::Offline);
        assert!(!health.schedulable);
        assert_eq!(node.availability, NodeMetricsAvailability::NotCollected);
        // Listed, not omitted — and its stored status is still `online`.
        assert_eq!(
            WorkerNode::find_by_id(&pool, lapsed.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkerNodeStatus::Online
        );
    }

    /// Analysis E7: with clustering off there is no `coordinator_id`, and the
    /// v5-over-hostname fallback has to survive a restart or the panel's
    /// persisted node selection breaks on every boot.
    #[tokio::test]
    async fn clustering_disabled_yields_one_node_with_a_stable_id() {
        let pool = test_pool().await;
        let disabled = config(None);
        assert!(!disabled.enabled && disabled.coordinator_id.is_none());

        let first = ClusterMetricsService::with_source(
            pool.clone(),
            &disabled,
            None,
            test_sampler_config(),
        );
        let second = ClusterMetricsService::with_source(
            pool.clone(),
            &disabled,
            None,
            test_sampler_config(),
        );
        // Two separately constructed services on one host: a `new_v4()` here
        // would differ, and the persisted selection would dangle.
        assert_eq!(first.coordinator_node_id(), second.coordinator_node_id());

        let snapshot = first.snapshot().await.unwrap();
        assert_eq!(snapshot.nodes.len(), 1);
        let node = &snapshot.nodes[&first.coordinator_node_id()];
        assert_eq!(node.role, NodeRole::Coordinator);
        // No worker row, so no cluster judgement to report.
        assert!(node.health.is_none());
        assert_eq!(
            snapshot.sample_interval_ms,
            test_sampler_config().interval_ms
        );
    }

    #[tokio::test]
    async fn the_collector_stops_when_the_last_subscriber_drops() {
        let pool = test_pool().await;
        let service = service(&pool, None);
        assert_eq!(service.subscriber_count(), 0);
        assert!(!service.is_collecting());

        let first = service.subscribe();
        assert_eq!(service.subscriber_count(), 1);
        assert!(service.is_collecting());

        let second = service.subscribe();
        assert_eq!(service.subscriber_count(), 2);
        drop(second);
        // Still one viewer: the collector keeps running.
        assert_eq!(service.subscriber_count(), 1);
        assert!(service.is_collecting());

        drop(first);
        assert_eq!(service.subscriber_count(), 0);

        for _ in 0..200 {
            if !service.is_collecting() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !service.is_collecting(),
            "collector kept running with no subscribers"
        );

        // And it can be restarted by a later viewer.
        let third = service.subscribe();
        assert!(service.is_collecting());
        drop(third);
    }

    /// A node whose row is deleted while its own request is in flight must not
    /// be re-inserted by the reply. The source below blocks mid-fetch so the
    /// deregistration lands in exactly that window.
    #[tokio::test]
    async fn a_node_deregistering_mid_poll_is_not_resurrected() {
        struct BlockingSource {
            started: Arc<Notify>,
            release: Arc<Notify>,
            captured_at: DateTime<Utc>,
        }

        #[async_trait]
        impl WorkerMetricsSource for BlockingSource {
            async fn fetch(
                &self,
                _worker_node_id: Uuid,
                _after: u64,
            ) -> Result<SampleBatch, MetricsFetchError> {
                self.started.notify_one();
                self.release.notified().await;
                Ok(batch(vec![sample(1, "think4", self.captured_at)]))
            }
        }

        let pool = test_pool().await;
        let worker = insert_worker(&pool, "think4", Utc::now() + TimeDelta::seconds(60)).await;
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let source = Arc::new(BlockingSource {
            started: started.clone(),
            release: release.clone(),
            captured_at: Utc::now(),
        });
        let service = service(&pool, Some(source));
        assert!(service.tracked_node_ids().is_empty());

        let round = {
            let service = service.clone();
            tokio::spawn(async move { service.collect_round().await })
        };

        started.notified().await;
        // The operator removes the worker while its reply is in flight.
        sqlx::query("DELETE FROM worker_nodes WHERE id = ?")
            .bind(worker.id)
            .execute(&pool)
            .await
            .unwrap();
        // A concurrent reader reconciles the node map against the new row set.
        service.snapshot().await.unwrap();
        assert!(!service.tracked_node_ids().contains(&worker.id));

        release.notify_one();
        round.await.unwrap();

        assert!(
            !service.tracked_node_ids().contains(&worker.id),
            "a late reply resurrected a deregistered node"
        );
        assert_eq!(
            service.tracked_node_ids(),
            vec![service.coordinator_node_id()]
        );
        assert!(
            !service
                .snapshot()
                .await
                .unwrap()
                .nodes
                .contains_key(&worker.id)
        );
    }

    /// Clarification C5: a node that stops reporting goes `Stale` with its
    /// readings retained, and once they are older than the retention window
    /// they are dropped rather than left next to a live graph.
    #[test]
    fn readings_go_stale_and_are_then_dropped() {
        let config = SamplerConfig {
            interval_ms: 1_000,
            retention: 3,
            max_processes: 5,
        };
        let start = Utc::now();
        let mut state = NodeState::new(1);

        apply_outcome(
            &mut state,
            0,
            Ok(batch(vec![sample(1, "think4", start)])),
            start,
            &config,
        );
        assert_eq!(state.availability, NodeMetricsAvailability::Available);
        assert_eq!(state.cursor, 1);

        // Contact, no fresh sample, well inside the flap window.
        let soon = start + TimeDelta::seconds(2);
        apply_outcome(&mut state, 1, Ok(empty_batch(1)), soon, &config);
        assert_eq!(state.availability, NodeMetricsAvailability::Available);
        assert!(state.latest.is_some());

        // Past `STALE_AFTER_TICKS`, the readings stay but stop claiming to be current.
        let later = start + TimeDelta::seconds(6);
        apply_outcome(&mut state, 1, Ok(empty_batch(1)), later, &config);
        assert_eq!(
            state.availability,
            NodeMetricsAvailability::Stale { since: start }
        );
        assert!(state.latest.is_some());
        assert_eq!(state.history.len(), 1);

        // Retention window is `interval_ms * retention` = 3s; the reading is 6s old.
        let mut nodes = HashMap::new();
        let id = Uuid::new_v4();
        nodes.insert(id, state);
        expire_locked(&mut nodes, later, TimeDelta::seconds(3));
        assert!(nodes[&id].latest.is_none());
        assert!(nodes[&id].history.is_empty());
        // Availability survives; only the numbers go.
        assert!(matches!(
            nodes[&id].availability,
            NodeMetricsAvailability::Stale { .. }
        ));
    }

    /// The retained ring is a function of the node count, never of uptime, and
    /// only the newest entry carries a process table.
    #[test]
    fn history_is_bounded_and_carries_no_process_table() {
        let config = SamplerConfig {
            interval_ms: 1_000,
            retention: 3,
            max_processes: 5,
        };
        let start = Utc::now();
        let mut state = NodeState::new(1);
        let mut samples = Vec::new();
        for sequence in 1..=6 {
            let mut entry = sample(
                sequence,
                "think4",
                start + TimeDelta::seconds(sequence as i64),
            );
            entry.processes = Some(Vec::new());
            samples.push(entry);
        }
        apply_outcome(&mut state, 0, Ok(batch(samples)), start, &config);

        assert_eq!(state.history.len(), 3);
        assert_eq!(
            state.history.iter().map(|s| s.sequence).collect::<Vec<_>>(),
            vec![4, 5, 6]
        );
        assert!(state.history.iter().all(|s| s.processes.is_none()));
        assert!(state.latest.as_ref().unwrap().processes.is_some());
        assert_eq!(state.cursor, 6);
    }
}
