//! Host metrics sampling for cluster nodes.
//!
//! A leaf crate: it reads `/proc`, derives rates against the previous raw
//! counters, and hands out a bounded ring of [`HostSample`]s. It knows nothing
//! about the database, the scheduler, or HTTP — which is what makes
//! "observability cannot influence lifecycle state" (constitution XIX) a
//! structural fact here rather than a promise.
//!
//! Two rules shape almost every signature in this crate:
//!
//! - **Absent is not zero.** A reading the host could not supply is `None`. A
//!   `0` would be a fabricated measurement, and a UI showing `0%` for a host
//!   whose `/proc/stat` was unreadable is a defect, not a rounding error.
//! - **A rate needs a predecessor.** Every derived value is computed against
//!   the previous *raw* counters, so a delayed tick scales correctly and the
//!   first sample of a sampler's life reports `None` rather than a spike.
//!
//! The split exists for testability: [`parse`], [`derive`], and [`redact`] are
//! pure and exhaustively tested against checked-in fixtures, and [`collect`] is
//! the only module that touches the filesystem.

pub mod collect;
pub mod derive;
pub mod parse;
pub mod redact;
pub mod sampler;
pub mod types;

pub use collect::{CollectError, Collector};
pub use sampler::MetricsSampler;
pub use types::{
    CoreBusy, CpuSample, DiskAlertThresholds, FilesystemSample, HostSample, MemorySample, NetworkSample,
    NodeMetricsAvailability, NodeRole, ProcessSample, SampleBatch, SamplerConfig,
};
