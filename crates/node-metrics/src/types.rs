//! Wire types for host metrics.
//!
//! Three properties of this module are load-bearing and easy to break:
//!
//! 1. **No `f32`/`f64` inside an internally-tagged enum.** `serde_json` is
//!    built workspace-wide with `preserve_order` (`Cargo.toml:44`), under which
//!    a float field of a `#[serde(tag = ...)]` variant fails to deserialize with
//!    `invalid type: map, expected f64`. Every percentage and load average
//!    therefore lives in a plain struct. [`tests::availability_round_trips`]
//!    guards it.
//! 2. **Every rate-derived or possibly-unreadable field is `Option`.** A
//!    non-`Option` numeric field is a promise that the value is always
//!    knowable; for host introspection that promise is almost never true.
//! 3. **`rename_all = "snake_case"` on every enum**, stated explicitly rather
//!    than inherited, because a casing mismatch between backend and frontend
//!    fails silently as a comparison that is never true.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One observation of one host at one instant.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct HostSample {
    /// Monotonic per sampler and never reused; the cursor consumers advance.
    pub sequence: u64,
    /// Read at sample time. This is the only source of a node's hostname —
    /// `ClusterConfig` carries no such field.
    pub hostname: String,
    pub captured_at: DateTime<Utc>,
    /// Real elapsed time since the previous sample, not the configured
    /// interval, so a delayed tick still produces a correctly scaled rate.
    /// `None` on the first sample.
    pub interval_ms: Option<u64>,
    pub uptime_seconds: Option<u64>,
    pub cpu: CpuSample,
    pub memory: MemorySample,
    /// `None` if the mount table was unreadable — distinct from "no
    /// filesystems", which is an empty `Vec`.
    pub filesystems: Option<Vec<FilesystemSample>>,
    /// `None` if `/proc/net/dev` was unreadable.
    pub networks: Option<Vec<NetworkSample>>,
    /// Populated on the newest sample only. Retained history entries carry
    /// `None`: the table is roughly 80% of a sample's size and nothing plots it
    /// over time. `Option` rather than an empty `Vec` because an empty list
    /// would be indistinguishable from "no processes were readable".
    pub processes: Option<Vec<ProcessSample>>,
    /// Human-readable notes about what could not be read. Bounded: collectors
    /// summarise repeated failures into one note rather than one per failure.
    pub degraded: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CpuSample {
    pub model: Option<String>,
    pub core_count: Option<u32>,
    /// `1 − Δidle/Δtotal`. `None` until a predecessor exists.
    pub total_busy_percent: Option<f32>,
    /// One entry per **online** core, each tagged with the kernel's own `cpuN`
    /// index.
    ///
    /// Tagged rather than positional because `/proc/stat` omits offline CPUs:
    /// with cpu1 offline the second entry is cpu2, and a reader labelling by
    /// array position would show cpu2's utilisation as "core 1". Replaced
    /// wholesale, never patched per element, so a core count change cannot
    /// misalign it.
    pub per_core_busy: Option<Vec<CoreBusy>>,
    pub load_1m: Option<f32>,
    pub load_5m: Option<f32>,
    pub load_15m: Option<f32>,
    pub frequency_mhz: Option<u32>,
    pub temperature_celsius: Option<f32>,
}

/// One core's derived busy percentage, tagged with the index the kernel gave
/// it.
///
/// A plain struct rather than a bare `f32` in a positional array: the label a
/// reader puts on the value is part of the reading, and on a host with an
/// offline CPU the position and the index disagree.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
pub struct CoreBusy {
    /// The `N` of `cpuN`.
    pub core: u32,
    pub busy_percent: f32,
}

/// Every field is `Option`: an unreadable `/proc/meminfo` yields `None`
/// throughout, not a machine that appears to have zero memory.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MemorySample {
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    /// `total − available`, deliberately not `total − free`: `MemFree` excludes
    /// reclaimable page cache and makes a healthy Linux box look full.
    pub used_bytes: Option<u64>,
    /// `Cached + SReclaimable`.
    pub cached_bytes: Option<u64>,
    pub swap_total_bytes: Option<u64>,
    pub swap_used_bytes: Option<u64>,
}

/// Keyed by `mount_point`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FilesystemSample {
    pub mount_point: String,
    pub device: String,
    pub fs_type: String,
    /// `None` if `statvfs` failed on an otherwise-listed mount — a stalled NFS
    /// server is the common case, and reporting it as 0 bytes would be a lie
    /// about the one filesystem this panel most exists to watch.
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

/// Keyed by `interface`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct NetworkSample {
    pub interface: String,
    pub rx_bytes_total: u64,
    pub tx_bytes_total: u64,
    /// `None` on the first sample and whenever the counter has gone backwards
    /// (interface reset), in which case a `degraded` note records it. A zero
    /// here would read as "no traffic", which is a different and false claim.
    pub rx_bytes_per_second: Option<u64>,
    pub tx_bytes_per_second: Option<u64>,
}

/// Identity is `(pid, start_ticks)`. PIDs are reused; the pair is not.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ProcessSample {
    pub pid: i32,
    /// `/proc/[pid]/stat` field 22. Identity only, never displayed.
    pub start_ticks: u64,
    pub name: String,
    pub user: Option<String>,
    /// Already redacted and truncated to 256 characters. The redactor runs
    /// inside the collector, so an unredacted command line cannot be held by
    /// this type at any point.
    pub command: String,
    /// `Δ(utime+stime) / (ticks_per_second × Δs) × 100`, capped at
    /// `core_count × 100`. `None` for a process first seen this sample.
    pub cpu_percent: Option<f32>,
    pub memory_bytes: Option<u64>,
    pub thread_count: Option<u32>,
}

/// The sampler's read interface, and the worker's response body.
///
/// A batch may contain zero, one, or many samples: the sampler and any poller
/// are independently phased, so jitter or a catch-up routinely produce 0 or 2
/// samples for a nominal one-tick poll, and a cold `after = 0` returns the whole
/// retained ring.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SampleBatch {
    /// Oldest → newest, those with `sequence > after`.
    pub samples: Vec<HostSample>,
    /// A cursor below this has fallen out of the ring; see [`SampleBatch::has_gap`].
    pub earliest_retained_sequence: u64,
    pub latest_sequence: u64,
}

impl SampleBatch {
    /// Whether samples between `after` and this batch were evicted before the
    /// consumer read them.
    ///
    /// A gap is reported rather than hidden. For metrics a gap is benign — a
    /// hole in a graph — so the consumer records the discontinuity and forces a
    /// resnapshot, unlike the execution journal where a gap is fatal.
    ///
    /// `after = 0` is a cold start, not a gap: the consumer has seen nothing
    /// and is asking for whatever is retained.
    pub fn has_gap(&self, after: u64) -> bool {
        after > 0 && after + 1 < self.earliest_retained_sequence
    }
}

/// Not operator-adjustable. Every value here is a clarification decision, and
/// exposing them as configuration would mean the memory bound and the patch
/// size stopped being facts about the system.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SamplerConfig {
    pub interval_ms: u64,
    /// Samples retained per node: 150 × 2s ≈ 5 minutes.
    pub retention: u32,
    /// Processes reported per sample, ranked by CPU.
    pub max_processes: u32,
}

impl SamplerConfig {
    pub const DEFAULT_INTERVAL_MS: u64 = 2_000;
    pub const DEFAULT_RETENTION: u32 = 150;
    pub const DEFAULT_MAX_PROCESSES: u32 = 15;

    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms)
    }
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            interval_ms: Self::DEFAULT_INTERVAL_MS,
            retention: Self::DEFAULT_RETENTION,
            max_processes: Self::DEFAULT_MAX_PROCESSES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    Coordinator,
    Worker,
}

/// Whether metrics could be read from a node — deliberately *not* whether the
/// node is healthy. A node that fails to report metrics is not offline, and a
/// node that reports them is not healthy; the cluster's own evidence channel
/// remains the only authority on both.
///
/// Internally tagged, mirroring `MountHealth` in `cluster-protocol`. **No
/// variant may carry an `f32`/`f64`** — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum NodeMetricsAvailability {
    /// Reporting normally.
    Available,
    /// Last contact succeeded, but no fresh sample since `since`. Retained
    /// readings stay on screen, de-emphasised, rather than vanishing.
    Stale { since: DateTime<Utc> },
    /// No poll has been attempted — the normal state of a worker while nobody
    /// is looking. Distinct from a failure.
    NotCollected,
    /// Not a Linux host.
    Unsupported { platform: String },
    /// Transport, auth, timeout, or oversized reply this cycle.
    Unreachable { reason: String },
    /// The worker build predates `GET /v1/metrics`.
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(value: &NodeMetricsAvailability) -> NodeMetricsAvailability {
        let encoded = serde_json::to_string(value).expect("serialize");
        serde_json::from_str(&encoded)
            .unwrap_or_else(|error| panic!("deserialize {encoded}: {error}"))
    }

    /// Guards the `preserve_order` hazard: under that feature an internally
    /// tagged enum with an `f32`/`f64` field serialises fine and then fails to
    /// deserialize with `invalid type: map, expected f64`. Adding a float to any
    /// variant below turns this test red instead of turning the drawer blank at
    /// runtime.
    #[test]
    fn availability_round_trips() {
        let cases = vec![
            NodeMetricsAvailability::Available,
            NodeMetricsAvailability::Stale {
                since: DateTime::parse_from_rfc3339("2026-08-01T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            },
            NodeMetricsAvailability::NotCollected,
            NodeMetricsAvailability::Unsupported {
                platform: "macos".to_string(),
            },
            NodeMetricsAvailability::Unreachable {
                reason: "connect timeout".to_string(),
            },
            NodeMetricsAvailability::NotImplemented,
        ];

        for case in cases {
            assert_eq!(round_trip(&case), case);
        }
    }

    #[test]
    fn availability_is_snake_case_tagged() {
        let encoded = serde_json::to_value(NodeMetricsAvailability::NotCollected).unwrap();
        assert_eq!(encoded, serde_json::json!({ "status": "not_collected" }));

        let encoded = serde_json::to_value(NodeMetricsAvailability::Unreachable {
            reason: "http 502".to_string(),
        })
        .unwrap();
        assert_eq!(
            encoded,
            serde_json::json!({ "status": "unreachable", "reason": "http 502" })
        );
    }

    #[test]
    fn node_role_is_snake_case() {
        assert_eq!(
            serde_json::to_value(NodeRole::Coordinator).unwrap(),
            serde_json::json!("coordinator")
        );
        assert_eq!(
            serde_json::to_value(NodeRole::Worker).unwrap(),
            serde_json::json!("worker")
        );
    }

    /// `HostSample` is float-bearing but is a plain struct, so `preserve_order`
    /// does not apply to it. This asserts that distinction holds in practice —
    /// the escape hatch invariant 1 depends on.
    #[test]
    fn float_bearing_struct_round_trips() {
        let sample = CpuSample {
            model: Some("Intel(R) Core(TM) i5-8500T CPU @ 2.10GHz".to_string()),
            core_count: Some(6),
            total_busy_percent: Some(12.5),
            per_core_busy_percent: Some(vec![1.5, 2.5, 3.0]),
            load_1m: Some(0.31),
            load_5m: Some(1.60),
            load_15m: Some(1.02),
            frequency_mhz: Some(900),
            temperature_celsius: Some(41.0),
        };
        let encoded = serde_json::to_string(&sample).unwrap();
        let decoded: CpuSample = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.total_busy_percent, Some(12.5));
        assert_eq!(decoded.load_1m, Some(0.31));
    }

    #[test]
    fn absent_readings_encode_as_null_not_zero() {
        let memory = MemorySample {
            total_bytes: None,
            available_bytes: None,
            used_bytes: None,
            cached_bytes: None,
            swap_total_bytes: None,
            swap_used_bytes: None,
        };
        let encoded = serde_json::to_value(&memory).unwrap();
        for (field, value) in encoded.as_object().unwrap() {
            assert!(value.is_null(), "{field} encoded as {value}, expected null");
        }
    }

    #[test]
    fn batch_reports_a_gap_only_when_samples_were_missed() {
        let batch = SampleBatch {
            samples: Vec::new(),
            earliest_retained_sequence: 51,
            latest_sequence: 200,
        };
        // A cold consumer asked for everything; nothing was missed.
        assert!(!batch.has_gap(0));
        // Its next needed sample is 51, which is still retained.
        assert!(!batch.has_gap(50));
        assert!(!batch.has_gap(120));
        // Its next needed sample is 50, which has been evicted.
        assert!(batch.has_gap(49));
    }

    #[test]
    fn sampler_config_defaults_match_the_clarifications() {
        let config = SamplerConfig::default();
        assert_eq!(config.interval(), Duration::from_secs(2));
        assert_eq!(config.retention, 150);
        assert_eq!(config.max_processes, 15);
    }
}
