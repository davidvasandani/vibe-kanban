//! The JSON-Patch builder for the cluster metrics stream.
//!
//! Pure: it takes the snapshot it last emitted and the snapshot it is emitting
//! now, and returns the operations between them. No socket, no clock, no
//! `Deployment` — the tick cadence, the 30s backstop and the resnapshot rules
//! are all expressible against a `generated_at` the caller supplies, which is
//! what makes them testable.
//!
//! Two rules here are load-bearing, and both fail *silently* if broken:
//!
//! - **A resnapshot targets `/nodes` and `/generated_at`, never the document
//!   root.** The consuming hook (`useJsonPatchWsStream`) mutates an Immer
//!   draft, and rfc6902 resolves an empty pointer to `parent === null`: a
//!   `replace` at `""` fails, `applyUpsertPatch` retries it as an `add`, and
//!   that attempts `null[''] = value`. The op compiles, ships, and never
//!   converges. See analysis E4.
//! - **Nodes are addressed by `node_id`, never by array index.** A worker
//!   registering or deregistering mid-stream would otherwise shift every index
//!   after it and land a `replace` on the wrong host.
//!
//! Patches are the optimisation; the resnapshot is the truth. Anything this
//! module cannot express exactly — a membership change, a replay gap, a worker
//! restart, a batch large enough that appending it individually is worse than
//! resending — falls back to a resnapshot rather than guessing.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use json_patch::{AddOperation, Patch, PatchOperation, RemoveOperation, ReplaceOperation};
use node_metrics::{HostSample, NodeMetricsAvailability};
use services::services::cluster::{ClusterMetricsSnapshot, MetricsNode, NodeHealth};
use uuid::Uuid;

/// The unconditional convergence backstop. A dropped broadcast is otherwise
/// undetectable by the client.
pub const RESNAPSHOT_INTERVAL: TimeDelta = TimeDelta::seconds(30);

/// Above this many appends in one tick, resending the node map is cheaper than
/// the operations that would describe it. The first poll after a connect
/// routinely returns a worker's whole retained ring.
const MAX_APPENDS_PER_TICK: usize = 8;

const NODES_PATH: &str = "/nodes";
const GENERATED_AT_PATH: &str = "/generated_at";
const DISK_ALERT_THRESHOLDS_PATH: &str = "/disk_alert_thresholds";

fn node_path(node_id: Uuid, suffix: &str) -> String {
    // A canonical UUID contains neither `~` nor `/`, so no pointer escaping is
    // required — but note this is a *key*, not an index.
    format!("{NODES_PATH}/{node_id}{suffix}")
}

fn value_of<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn replace(path: String, value: serde_json::Value) -> PatchOperation {
    PatchOperation::Replace(ReplaceOperation {
        path: path
            .try_into()
            .expect("cluster metrics pointer should be valid"),
        value,
    })
}

fn add(path: String, value: serde_json::Value) -> PatchOperation {
    PatchOperation::Add(AddOperation {
        path: path
            .try_into()
            .expect("cluster metrics pointer should be valid"),
        value,
    })
}

fn remove(path: String) -> PatchOperation {
    PatchOperation::Remove(RemoveOperation {
        path: path
            .try_into()
            .expect("cluster metrics pointer should be valid"),
    })
}

/// Everything the builder has to remember about one node to describe the next
/// tick. Deliberately not the samples themselves: the builder's own footprint
/// must not grow with uptime either.
#[derive(Debug, Clone, PartialEq)]
struct NodeCursor {
    latest_sequence: Option<u64>,
    history_len: usize,
    history_last_sequence: Option<u64>,
    availability: NodeMetricsAvailability,
    health: Option<NodeHealth>,
}

impl NodeCursor {
    fn of(node: &MetricsNode) -> Self {
        Self {
            latest_sequence: node.latest.as_ref().map(|sample| sample.sequence),
            history_len: node.history.len(),
            history_last_sequence: node.history.last().map(|sample| sample.sequence),
            availability: node.availability.clone(),
            health: node.health.clone(),
        }
    }
}

/// Per-node result: the operations, and how many samples they appended.
struct NodeOperations {
    ops: Vec<PatchOperation>,
    appends: usize,
}

/// Tracks what one connected client has been told.
///
/// One per socket: two viewers are at different points in the stream, and
/// sharing a builder between them would send each the other's deltas.
#[derive(Debug, Default)]
pub struct MetricsPatchBuilder {
    nodes: HashMap<Uuid, NodeCursor>,
    last_resnapshot_at: Option<DateTime<Utc>>,
}

impl MetricsPatchBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The operations taking this client from what it has been sent to
    /// `snapshot`.
    ///
    /// Empty when nothing changed — a tick in which no node produced a sample
    /// costs zero bytes on the wire, which is the point of the whole scheme.
    pub fn next(&mut self, snapshot: &ClusterMetricsSnapshot) -> Patch {
        let patch = match self.incremental(snapshot) {
            Some(ops) => Patch(ops),
            None => {
                self.last_resnapshot_at = Some(snapshot.generated_at);
                resnapshot(snapshot)
            }
        };
        self.nodes = snapshot
            .nodes
            .iter()
            .map(|(id, node)| (*id, NodeCursor::of(node)))
            .collect();
        patch
    }

    /// `None` when this tick is not expressible as a patch and the
    /// level-triggered resnapshot has to take over.
    fn incremental(&self, snapshot: &ClusterMetricsSnapshot) -> Option<Vec<PatchOperation>> {
        // Nothing has been sent yet: the client's document is the seeded
        // `{ nodes: {}, generated_at: null }`.
        let last_resnapshot_at = self.last_resnapshot_at?;
        if snapshot.generated_at - last_resnapshot_at >= RESNAPSHOT_INTERVAL {
            return None;
        }
        // Any membership change. Node-keyed patches could express an add or a
        // remove, but the whole map at this size is a few kilobytes and the
        // resnapshot is the path that is exercised on every connect.
        if self.nodes.len() != snapshot.nodes.len()
            || !snapshot.nodes.keys().all(|id| self.nodes.contains_key(id))
        {
            return None;
        }

        let mut ops = Vec::new();
        let mut appends = 0usize;
        for (id, node) in &snapshot.nodes {
            let cursor = self.nodes.get(id)?;
            let node_ops = node_operations(*id, cursor, node)?;
            appends += node_ops.appends;
            ops.extend(node_ops.ops);
        }
        if appends > MAX_APPENDS_PER_TICK {
            return None;
        }
        Some(ops)
    }
}

/// `replace /nodes` + `replace /generated_at` — **never** a root `replace`.
fn resnapshot(snapshot: &ClusterMetricsSnapshot) -> Patch {
    Patch(vec![
        replace(NODES_PATH.to_owned(), value_of(&snapshot.nodes)),
        replace(
            GENERATED_AT_PATH.to_owned(),
            value_of(&snapshot.generated_at),
        ),
        replace(
            DISK_ALERT_THRESHOLDS_PATH.to_owned(),
            value_of(&snapshot.disk_alert_thresholds),
        ),
    ])
}

/// `None` if this node's transition cannot be described exactly, which forces
/// the whole tick to resnapshot rather than emit a patch that is subtly wrong.
fn node_operations(
    node_id: Uuid,
    cursor: &NodeCursor,
    node: &MetricsNode,
) -> Option<NodeOperations> {
    // Readings aged out of the retention window: `latest` went away, which no
    // append describes.
    if cursor.latest_sequence.is_some() && node.latest.is_none() {
        return None;
    }
    // The worker restarted and its sequences began again below our cursor.
    if let (Some(previous), Some(current)) = (
        cursor.latest_sequence,
        node.latest.as_ref().map(|sample| sample.sequence),
    ) && current < previous
    {
        return None;
    }
    // A replay gap: the client's newest retained sample is no longer adjacent
    // to anything the server still holds, so appending would draw a straight
    // line across missing time.
    if let (Some(previous), Some(oldest)) = (
        cursor.history_last_sequence,
        node.history.first().map(|sample| sample.sequence),
    ) && oldest > previous + 1
    {
        return None;
    }

    let fresh: Vec<&HostSample> = match cursor.history_last_sequence {
        Some(previous) => node
            .history
            .iter()
            .filter(|sample| sample.sequence > previous)
            .collect(),
        None => node.history.iter().collect(),
    };
    let appends = fresh.len();
    // Derived, not assumed: the client's history has to end up exactly as long
    // as the server's. A negative or oversized eviction count means the two
    // have diverged in a way this builder cannot reconcile.
    let evictions = (cursor.history_len + appends).checked_sub(node.history.len())?;
    if evictions > cursor.history_len {
        return None;
    }

    let mut ops = Vec::new();
    if appends == 0 {
        // A node that returned nothing this tick emits nothing — not an empty
        // append, not a redundant `replace` of unchanged data.
        if evictions > 0 {
            return None;
        }
    } else {
        ops.push(replace(
            node_path(node_id, "/latest"),
            value_of(&node.latest),
        ));
        let mut remaining = evictions;
        for sample in fresh {
            ops.push(add(node_path(node_id, "/history/-"), value_of(sample)));
            if remaining > 0 {
                ops.push(remove(node_path(node_id, "/history/0")));
                remaining -= 1;
            }
        }
    }

    // A per-node collection failure is not a stream error: it is this node's
    // `availability`, patched like any other field.
    if cursor.availability != node.availability {
        ops.push(replace(
            node_path(node_id, "/availability"),
            value_of(&node.availability),
        ));
    }
    if cursor.health != node.health {
        ops.push(replace(
            node_path(node_id, "/health"),
            value_of(&node.health),
        ));
    }

    Some(NodeOperations { ops, appends })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use db::models::worker_node::{WorkerMountStatus, WorkerNodeStatus};
    use node_metrics::{CpuSample, MemorySample, NodeRole, SamplerConfig};

    use super::*;

    fn paths(patch: &Patch) -> Vec<String> {
        patch
            .0
            .iter()
            .map(|op| match op {
                PatchOperation::Add(op) => format!("add {}", op.path),
                PatchOperation::Remove(op) => format!("remove {}", op.path),
                PatchOperation::Replace(op) => format!("replace {}", op.path),
                other => format!("other {:?}", other),
            })
            .collect()
    }

    fn sample(sequence: u64) -> HostSample {
        HostSample {
            sequence,
            hostname: "think4".into(),
            captured_at: DateTime::UNIX_EPOCH + TimeDelta::seconds(sequence as i64),
            interval_ms: Some(2_000),
            uptime_seconds: Some(3_600),
            cpu: CpuSample {
                model: None,
                core_count: Some(4),
                total_busy_percent: Some(11.0),
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

    fn node(sequences: &[u64]) -> MetricsNode {
        MetricsNode {
            node_id: Uuid::nil(),
            hostname: "think4".into(),
            role: NodeRole::Worker,
            health: Some(NodeHealth {
                status: WorkerNodeStatus::Online,
                mount_status: Some(WorkerMountStatus::Healthy),
                lease_expires_at: None,
                schedulable: true,
            }),
            availability: NodeMetricsAvailability::Available,
            latest: sequences.last().map(|sequence| sample(*sequence)),
            history: sequences.iter().map(|sequence| sample(*sequence)).collect(),
            last_contact_at: None,
        }
    }

    struct Fixture {
        generated_at: DateTime<Utc>,
        nodes: BTreeMap<Uuid, MetricsNode>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                generated_at: DateTime::UNIX_EPOCH,
                nodes: BTreeMap::new(),
            }
        }

        fn with(mut self, id: Uuid, sequences: &[u64]) -> Self {
            let mut node = node(sequences);
            node.node_id = id;
            self.nodes.insert(id, node);
            self
        }

        fn tick(&mut self, seconds: i64) {
            self.generated_at += TimeDelta::seconds(seconds);
        }

        fn snapshot(&self) -> ClusterMetricsSnapshot {
            ClusterMetricsSnapshot {
                nodes: self.nodes.clone(),
                generated_at: self.generated_at,
                sample_interval_ms: SamplerConfig::DEFAULT_INTERVAL_MS,
                disk_alert_thresholds: node_metrics::types::DiskAlertThresholds::default(),
            }
        }
    }

    /// The client's seed, from `initialData()`.
    fn seeded_document() -> serde_json::Value {
        serde_json::json!({
            "nodes": {},
            "generated_at": serde_json::Value::Null,
            "sample_interval_ms": SamplerConfig::DEFAULT_INTERVAL_MS,
            "disk_alert_thresholds": node_metrics::types::DiskAlertThresholds::default(),
        })
    }

    fn apply(document: &mut serde_json::Value, patch: &Patch) {
        json_patch::patch(document, &patch.0).expect("client should be able to apply the patch");
    }

    /// Analysis E4. A `replace` at `""` compiles, ships, and silently never
    /// lands, taking the entire convergence story with it.
    #[test]
    fn the_resnapshot_targets_named_subpaths_not_the_document_root() {
        let fixture = Fixture::new().with(Uuid::from_u128(1), &[1, 2]);
        let patch = MetricsPatchBuilder::new().next(&fixture.snapshot());

        assert_eq!(
            paths(&patch),
            [
                "replace /nodes",
                "replace /generated_at",
                "replace /disk_alert_thresholds",
            ]
        );
        assert!(
            patch.0.iter().all(|op| match op {
                PatchOperation::Replace(op) => !op.path.is_root(),
                _ => true,
            }),
            "a root pointer cannot be applied by useJsonPatchWsStream"
        );

        // And it lands on the document the client actually seeds.
        let mut document = seeded_document();
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    #[test]
    fn a_zero_sample_tick_emits_nothing() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        builder.next(&fixture.snapshot());

        fixture.tick(2);
        let patch = builder.next(&fixture.snapshot());
        assert!(patch.0.is_empty(), "{:?}", paths(&patch));
    }

    #[test]
    fn a_two_sample_tick_appends_and_evicts_once_per_sample() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[1, 2, 3]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        // The ring is full at three, so two arrivals evict two.
        fixture.tick(2);
        fixture = fixture.with(id, &[3, 4, 5]);
        let patch = builder.next(&fixture.snapshot());
        assert_eq!(
            paths(&patch),
            [
                format!("replace /nodes/{id}/latest"),
                format!("add /nodes/{id}/history/-"),
                format!("remove /nodes/{id}/history/0"),
                format!("add /nodes/{id}/history/-"),
                format!("remove /nodes/{id}/history/0"),
            ]
        );

        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    #[test]
    fn a_silent_node_is_untouched_while_its_peer_advances() {
        let quiet = Uuid::from_u128(1);
        let busy = Uuid::from_u128(2);
        let mut fixture = Fixture::new().with(quiet, &[1, 2]).with(busy, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        fixture = fixture.with(busy, &[1, 2, 3]);
        let patch = builder.next(&fixture.snapshot());

        assert!(
            paths(&patch)
                .iter()
                .all(|path| path.contains(&busy.to_string())),
            "{:?}",
            paths(&patch)
        );
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    #[test]
    fn a_worker_added_between_ticks_resnapshots_without_corrupting_its_peer() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let mut fixture = Fixture::new().with(first, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        fixture = fixture.with(second, &[1]);
        let patch = builder.next(&fixture.snapshot());
        assert_eq!(paths(&patch), ["replace /nodes", "replace /generated_at"]);

        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
        assert_eq!(
            document["nodes"][first.to_string()]["history"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn a_worker_removed_between_ticks_resnapshots() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let mut fixture = Fixture::new().with(first, &[1, 2]).with(second, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        fixture.nodes.remove(&second);
        let patch = builder.next(&fixture.snapshot());
        assert_eq!(paths(&patch), ["replace /nodes", "replace /generated_at"]);

        apply(&mut document, &patch);
        assert!(document["nodes"].get(second.to_string()).is_none());
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    /// The first poll after a connect returns the worker's whole retained ring.
    #[test]
    fn a_cold_start_batch_resnapshots_instead_of_appending() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        let ring: Vec<u64> = (1..=150).collect();
        fixture = fixture.with(id, &ring);
        let patch = builder.next(&fixture.snapshot());

        assert_eq!(paths(&patch), ["replace /nodes", "replace /generated_at"]);
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    #[test]
    fn a_replay_gap_resnapshots() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[1, 2, 3]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        // Samples 4..=8 were evicted before we read them.
        fixture.tick(2);
        fixture = fixture.with(id, &[9, 10, 11]);
        let patch = builder.next(&fixture.snapshot());

        assert_eq!(paths(&patch), ["replace /nodes", "replace /generated_at"]);
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    /// A worker that restarted begins its sequences again below our cursor.
    #[test]
    fn a_worker_restart_resnapshots() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[40, 41, 42]);
        let mut builder = MetricsPatchBuilder::new();
        builder.next(&fixture.snapshot());

        fixture.tick(2);
        fixture = fixture.with(id, &[1]);
        assert_eq!(
            paths(&builder.next(&fixture.snapshot())),
            ["replace /nodes", "replace /generated_at"]
        );
    }

    /// Readings aged out of the retention window: `latest` goes to `null`,
    /// which no append describes.
    #[test]
    fn expired_readings_resnapshot() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        let mut expired = node(&[]);
        expired.node_id = id;
        expired.availability = NodeMetricsAvailability::Stale {
            since: fixture.generated_at,
        };
        fixture.nodes.insert(id, expired);
        let patch = builder.next(&fixture.snapshot());
        assert_eq!(paths(&patch), ["replace /nodes", "replace /generated_at"]);
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    #[test]
    fn the_thirty_second_backstop_resnapshots_even_when_nothing_changed() {
        let id = Uuid::from_u128(1);
        let mut fixture = Fixture::new().with(id, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        builder.next(&fixture.snapshot());

        fixture.tick(29);
        assert!(builder.next(&fixture.snapshot()).0.is_empty());

        fixture.tick(1);
        assert_eq!(
            paths(&builder.next(&fixture.snapshot())),
            ["replace /nodes", "replace /generated_at"]
        );
    }

    #[test]
    fn a_failure_becomes_that_nodes_availability_and_nothing_else() {
        let broken = Uuid::from_u128(1);
        let fine = Uuid::from_u128(2);
        let mut fixture = Fixture::new().with(broken, &[1, 2]).with(fine, &[1, 2]);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        fixture.tick(2);
        let node = fixture.nodes.get_mut(&broken).unwrap();
        node.availability = NodeMetricsAvailability::Unreachable {
            reason: "connection refused".into(),
        };
        node.health = Some(NodeHealth {
            status: WorkerNodeStatus::Offline,
            mount_status: Some(WorkerMountStatus::Healthy),
            lease_expires_at: None,
            schedulable: false,
        });

        let patch = builder.next(&fixture.snapshot());
        assert_eq!(
            paths(&patch),
            [
                format!("replace /nodes/{broken}/availability"),
                format!("replace /nodes/{broken}/health"),
            ]
        );
        apply(&mut document, &patch);
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
    }

    /// The bound the whole scheme exists to provide: an hour of streaming costs
    /// the same per tick as the first minute, and the builder's own state does
    /// not grow either.
    #[test]
    fn the_payload_does_not_grow_with_uptime() {
        let id = Uuid::from_u128(1);
        let ring: Vec<u64> = (1..=150).collect();
        let mut fixture = Fixture::new().with(id, &ring);
        let mut builder = MetricsPatchBuilder::new();
        let mut document = seeded_document();
        apply(&mut document, &builder.next(&fixture.snapshot()));

        let mut sizes = Vec::new();
        // 1800 ticks at 2s is an hour; the 30s backstop fires every 15th.
        for tick in 1..=1800u64 {
            fixture.tick(2);
            let window: Vec<u64> = (tick + 1..=tick + 150).collect();
            fixture = fixture.with(id, &window);
            let patch = builder.next(&fixture.snapshot());
            apply(&mut document, &patch);
            if patch.0.len() == 2 {
                continue; // the periodic resnapshot
            }
            assert_eq!(
                paths(&patch),
                [
                    format!("replace /nodes/{id}/latest"),
                    format!("add /nodes/{id}/history/-"),
                    format!("remove /nodes/{id}/history/0"),
                ]
            );
            sizes.push(serde_json::to_string(&patch).unwrap().len());
        }

        assert!(sizes.len() > 1000);
        // Every tick carries exactly one sample plus two pointers. The only
        // drift an hour of streaming is allowed to produce is the decimal width
        // of a sequence number — not one byte of retained history, which by the
        // end is 150 samples the client already has.
        let smallest = *sizes.iter().min().unwrap();
        let largest = *sizes.iter().max().unwrap();
        assert!(
            largest - smallest < 16,
            "per-tick payload grew with uptime: {smallest}..{largest} bytes"
        );
        // The client's document is still exactly the server's snapshot, and
        // still 150 entries long.
        assert_eq!(document["nodes"], value_of(&fixture.snapshot().nodes));
        assert_eq!(
            document["nodes"][id.to_string()]["history"]
                .as_array()
                .unwrap()
                .len(),
            150
        );
    }
}
