//! The only module in this crate that touches the filesystem.
//!
//! Everything else is pure so it can be tested exhaustively against fixtures;
//! this module is the narrow, deliberately boring boundary where real `/proc`
//! and `/sys` reads happen.
//!
//! Two error-handling rules matter more than the reading itself:
//!
//! - **A failed read is not a zero.** Every unreadable source yields `None`
//!   plus a `degraded` note. The panel says "we could not read this"; it never
//!   invents a measurement.
//! - **"Gone" and "unreadable" are different facts.** A `/proc/[pid]` entry
//!   that vanished mid-walk is routine — the process exited between the
//!   `read_dir` and the open. A permission error is not. Collapsing them with
//!   `filter_map(|e| e.ok())` turns "we could not read this" into "it was not
//!   there", which is exactly the trap recorded in the knowledge base.

use chrono::Utc;
use thiserror::Error;

use crate::{
    derive::Counters,
    types::{HostSample, SamplerConfig},
};

#[derive(Debug, Error)]
pub enum CollectError {
    /// `/proc/stat` is unreadable or unparsable. Without it there is no CPU
    /// baseline at all, and a sample that reported every core as absent would
    /// be indistinguishable from a host with no CPU — so this is the one
    /// failure that aborts a sample rather than degrading it.
    #[error("/proc/stat unavailable: {0}")]
    ProcStatUnavailable(String),
    /// Collection is not implemented for this platform.
    #[error("host metrics are not supported on {0}")]
    UnsupportedPlatform(String),
}

/// Reads a host once per call.
///
/// Holds no mutable state: the previous [`Counters`] are passed in and the new
/// ones returned, so the caller owns continuity and a `Collector` can be shared
/// or rebuilt freely.
#[derive(Debug, Clone)]
pub struct Collector {
    config: SamplerConfig,
}

/// A sample plus the raw counters the next one must derive against.
#[derive(Debug, Clone)]
pub struct Collected {
    pub sample: HostSample,
    pub counters: Counters,
}

impl Collector {
    pub fn new(config: SamplerConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SamplerConfig {
        &self.config
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        collections::BTreeMap,
        ffi::CString,
        fs,
        io::ErrorKind,
        path::{Path, PathBuf},
    };

    use super::*;
    use crate::{
        derive::{self, ProcessKey},
        parse,
        redact::redact_command,
        types::{FilesystemSample, ProcessSample},
    };

    const PROC: &str = "/proc";
    const THERMAL: &str = "/sys/class/thermal";

    impl Collector {
        /// Read the host once and derive rates against `previous`.
        pub fn collect(
            &self,
            previous: &Counters,
            sequence: u64,
        ) -> Result<Collected, CollectError> {
            let captured_at = Utc::now();
            let mut degraded = Vec::new();

            let stat_raw = read_to_string(&proc_path("stat"))
                .map_err(|error| CollectError::ProcStatUnavailable(error.to_string()))?;
            let cpu_stat = parse::parse_proc_stat(&stat_raw).ok_or_else(|| {
                CollectError::ProcStatUnavailable("could not parse aggregate cpu line".into())
            })?;

            let elapsed_ms = derive::elapsed_ms(previous.captured_at, captured_at);
            let busy = derive::derive_cpu_busy(previous.cpu.as_ref(), &cpu_stat);

            let load = optional(
                read_to_string(&proc_path("loadavg"))
                    .ok()
                    .as_deref()
                    .and_then(parse::parse_loadavg),
                "load averages",
                &mut degraded,
            );
            let cpu_info = read_to_string(&proc_path("cpuinfo"))
                .ok()
                .map(|raw| parse::parse_cpuinfo(&raw))
                .unwrap_or_default();

            let memory = match read_to_string(&proc_path("meminfo")) {
                Ok(raw) => parse::parse_meminfo(&raw),
                Err(error) => {
                    degraded.push(format!("memory unavailable: {error}"));
                    parse::parse_meminfo("")
                }
            };

            let networks = match read_to_string(&proc_path("net/dev")) {
                Ok(raw) => {
                    let counters = parse::parse_net_dev(&raw);
                    let samples = derive::derive_networks(
                        &previous.networks,
                        &counters,
                        elapsed_ms,
                        &mut degraded,
                    );
                    let carried = counters
                        .into_iter()
                        .map(|entry| (entry.interface.clone(), entry))
                        .collect();
                    (Some(samples), carried)
                }
                Err(error) => {
                    degraded.push(format!("network counters unavailable: {error}"));
                    (None, BTreeMap::new())
                }
            };

            let filesystems = match read_to_string(&proc_path("self/mounts")) {
                Ok(raw) => Some(collect_filesystems(
                    &parse::parse_mounts(&raw),
                    &mut degraded,
                )),
                Err(error) => {
                    degraded.push(format!("mount table unavailable: {error}"));
                    None
                }
            };

            let core_count = (!cpu_stat.per_core.is_empty())
                .then(|| u32::try_from(cpu_stat.per_core.len()).unwrap_or(u32::MAX));

            let (processes, process_ticks) =
                self.collect_processes(previous, elapsed_ms, core_count, &mut degraded);

            let sample = HostSample {
                sequence,
                hostname: hostname(),
                captured_at,
                interval_ms: elapsed_ms,
                uptime_seconds: read_to_string(&proc_path("uptime"))
                    .ok()
                    .as_deref()
                    .and_then(parse::parse_uptime),
                cpu: crate::types::CpuSample {
                    model: cpu_info.model,
                    core_count,
                    total_busy_percent: busy.total_percent,
                    per_core_busy_percent: busy.per_core_percent,
                    load_1m: load.map(|l| l.one),
                    load_5m: load.map(|l| l.five),
                    load_15m: load.map(|l| l.fifteen),
                    frequency_mhz: cpu_info.frequency_mhz,
                    temperature_celsius: temperature_celsius(),
                },
                memory,
                filesystems,
                networks: networks.0,
                processes: Some(processes),
                degraded,
            };

            Ok(Collected {
                sample,
                counters: Counters {
                    captured_at: Some(captured_at),
                    cpu: Some(cpu_stat),
                    networks: networks.1,
                    processes: process_ticks,
                },
            })
        }

        /// Walk `/proc/[pid]`, ranked by CPU and truncated to `max_processes`.
        ///
        /// Returns the visible processes plus the busy-tick map the next sample
        /// derives against. The tick map covers **every** process seen, not just
        /// the reported top-N: a process that was 16th last tick and 1st this
        /// tick still needs its predecessor, or it would report `None` forever.
        fn collect_processes(
            &self,
            previous: &Counters,
            elapsed_ms: Option<u64>,
            core_count: Option<u32>,
            degraded: &mut Vec<String>,
        ) -> (Vec<ProcessSample>, BTreeMap<ProcessKey, u64>) {
            let ticks_per_second = clock_ticks_per_second();
            let users = read_to_string(Path::new("/etc/passwd"))
                .map(|raw| parse::parse_passwd(&raw))
                .unwrap_or_default();

            let entries = match fs::read_dir(PROC) {
                Ok(entries) => entries,
                Err(error) => {
                    degraded.push(format!("process table unavailable: {error}"));
                    return (Vec::new(), BTreeMap::new());
                }
            };

            let mut samples = Vec::new();
            let mut ticks = BTreeMap::new();
            // Counted rather than pushed per failure: one unreadable process on
            // a busy host would otherwise emit hundreds of notes into every
            // sample of a live stream.
            let mut unreadable = 0_usize;

            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        unreadable += 1;
                        continue;
                    }
                };
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse::<i32>().ok())
                else {
                    continue;
                };

                let dir = entry.path();
                let stat_raw = match read_to_string(&dir.join("stat")) {
                    Ok(raw) => raw,
                    // The process exited between the read_dir and this open.
                    // Routine on any busy host, and not a degradation.
                    Err(error) if error.kind() == ErrorKind::NotFound => continue,
                    Err(_) => {
                        unreadable += 1;
                        continue;
                    }
                };
                let Some(stat) = parse::parse_process_stat(&stat_raw) else {
                    unreadable += 1;
                    continue;
                };

                let key: ProcessKey = (stat.pid, stat.start_ticks);
                let busy = stat.busy_ticks();
                ticks.insert(key, busy);

                let status = read_to_string(&dir.join("status"))
                    .map(|raw| parse::parse_process_status(&raw))
                    .unwrap_or_default();
                let command = read_to_string(&dir.join("cmdline"))
                    .ok()
                    .as_deref()
                    .and_then(parse::parse_cmdline)
                    // A kernel thread has an empty cmdline; its `comm` in
                    // brackets is the conventional rendering.
                    .map(|raw| redact_command(&raw))
                    .unwrap_or_else(|| format!("[{}]", stat.name));

                samples.push(ProcessSample {
                    pid: stat.pid,
                    start_ticks: stat.start_ticks,
                    name: stat.name,
                    user: status.uid.and_then(|uid| users.get(&uid).cloned()),
                    command,
                    cpu_percent: derive::derive_process_cpu(
                        previous.processes.get(&key).copied(),
                        busy,
                        elapsed_ms,
                        ticks_per_second,
                        core_count,
                    ),
                    memory_bytes: status.memory_bytes,
                    thread_count: status.thread_count,
                });
                let _ = pid;
            }

            if unreadable > 0 {
                degraded.push(format!("{unreadable} process entries could not be read"));
            }

            // `None` sorts last: a process first seen this sample has no
            // measured rate and must not outrank a measured one.
            samples.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.pid.cmp(&b.pid))
            });
            samples.truncate(self.config.max_processes as usize);

            (samples, ticks)
        }
    }

    fn proc_path(relative: &str) -> PathBuf {
        Path::new(PROC).join(relative)
    }

    fn read_to_string(path: &Path) -> std::io::Result<String> {
        fs::read_to_string(path)
    }

    fn optional<T>(value: Option<T>, what: &str, degraded: &mut Vec<String>) -> Option<T> {
        if value.is_none() {
            degraded.push(format!("{what} unavailable"));
        }
        value
    }

    fn hostname() -> String {
        read_to_string(&proc_path("sys/kernel/hostname"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// `sysconf(_SC_CLK_TCK)` — the divisor for every process CPU percentage.
    /// Falls back to the near-universal 100 rather than returning an error,
    /// because a wrong-by-a-constant-factor percentage is still ordinally
    /// correct and the panel's job is ranking.
    fn clock_ticks_per_second() -> u64 {
        // SAFETY: `sysconf` is a pure lookup with no preconditions.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if ticks > 0 { ticks as u64 } else { 100 }
    }

    /// Size a filesystem with `statvfs`.
    ///
    /// A stalled NFS server makes this block or fail; failure yields `None`
    /// rather than zeroes, because the shared mount is the one filesystem this
    /// panel most exists to watch and "0 bytes free" would be a false alarm.
    fn collect_filesystems(
        mounts: &[parse::MountEntry],
        degraded: &mut Vec<String>,
    ) -> Vec<FilesystemSample> {
        let mut unreadable = 0_usize;
        let samples = mounts
            .iter()
            .map(|mount| {
                let sizes = statvfs(&mount.mount_point);
                if sizes.is_none() {
                    unreadable += 1;
                }
                let (total, available, used) = sizes.unzip3();
                FilesystemSample {
                    mount_point: mount.mount_point.clone(),
                    device: mount.device.clone(),
                    fs_type: mount.fs_type.clone(),
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                }
            })
            .collect();
        if unreadable > 0 {
            degraded.push(format!("{unreadable} filesystems could not be sized"));
        }
        samples
    }

    /// `(total, available, used)` in bytes.
    fn statvfs(mount_point: &str) -> Option<(u64, u64, u64)> {
        let path = CString::new(mount_point).ok()?;
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: `path` is a valid NUL-terminated string and `stats` is a
        // correctly sized, writable `statvfs`.
        let rc = unsafe { libc::statvfs(path.as_ptr(), &mut stats) };
        if rc != 0 {
            return None;
        }
        let block = stats.f_frsize as u64;
        let total = stats.f_blocks as u64 * block;
        // `f_bavail` (available to unprivileged users) rather than `f_bfree`,
        // which counts root-reserved blocks a workload cannot actually use.
        let available = stats.f_bavail as u64 * block;
        let used = total.saturating_sub(stats.f_bfree as u64 * block);
        Some((total, available, used))
    }

    trait Unzip3 {
        fn unzip3(self) -> (Option<u64>, Option<u64>, Option<u64>);
    }

    impl Unzip3 for Option<(u64, u64, u64)> {
        fn unzip3(self) -> (Option<u64>, Option<u64>, Option<u64>) {
            match self {
                Some((a, b, c)) => (Some(a), Some(b), Some(c)),
                None => (None, None, None),
            }
        }
    }

    /// First thermal zone, preferring a package/core sensor over whatever the
    /// firmware happens to list first (often a battery or wireless sensor).
    fn temperature_celsius() -> Option<f32> {
        let mut fallback = None;
        let entries = fs::read_dir(THERMAL).ok()?;
        for entry in entries.flatten() {
            let dir = entry.path();
            let Ok(kind) = read_to_string(&dir.join("type")) else {
                continue;
            };
            let Some(millidegrees) = read_to_string(&dir.join("temp"))
                .ok()
                .and_then(|raw| raw.trim().parse::<i64>().ok())
            else {
                continue;
            };
            let celsius = millidegrees as f32 / 1000.0;
            match kind.trim() {
                "x86_pkg_temp" | "coretemp" => return Some(celsius),
                _ => fallback.get_or_insert(celsius),
            };
        }
        fallback
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;

    impl Collector {
        /// Non-Linux hosts report `Unsupported` rather than an empty sample.
        ///
        /// The distinction is the whole point: a zeroed sample from a macOS box
        /// would render as a perfectly idle machine, which is a fabricated
        /// reading. The caller maps this error onto
        /// [`crate::types::NodeMetricsAvailability::Unsupported`].
        pub fn collect(
            &self,
            _previous: &Counters,
            _sequence: u64,
        ) -> Result<Collected, CollectError> {
            Err(CollectError::UnsupportedPlatform(
                std::env::consts::OS.to_string(),
            ))
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// A smoke test against the real host. The parsers are covered
    /// exhaustively by fixtures; what this asserts is that the wiring reads the
    /// files it thinks it reads.
    #[test]
    fn collects_a_first_sample_from_the_live_host() {
        let collector = Collector::new(SamplerConfig::default());
        let collected = collector
            .collect(&Counters::default(), 1)
            .expect("/proc/stat should be readable on a Linux test host");

        assert_eq!(collected.sample.sequence, 1);
        assert!(!collected.sample.hostname.is_empty());
        assert!(collected.sample.cpu.core_count.unwrap_or(0) >= 1);
        assert!(collected.counters.cpu.is_some());
    }

    /// FR-7, at the boundary where it is easiest to get wrong: the first sample
    /// of a sampler's life has no predecessor, so every rate must be absent
    /// rather than zero.
    #[test]
    fn first_sample_reports_no_rates() {
        let collector = Collector::new(SamplerConfig::default());
        let collected = collector.collect(&Counters::default(), 1).unwrap();

        assert_eq!(collected.sample.interval_ms, None);
        assert_eq!(collected.sample.cpu.total_busy_percent, None);
        assert_eq!(collected.sample.cpu.per_core_busy_percent, None);
        for network in collected.sample.networks.iter().flatten() {
            assert_eq!(network.rx_bytes_per_second, None);
            assert_eq!(network.tx_bytes_per_second, None);
        }
        for process in collected.sample.processes.iter().flatten() {
            assert_eq!(process.cpu_percent, None);
        }
    }

    /// The second sample, derived against the first, must produce real rates.
    #[test]
    fn second_sample_derives_rates_against_the_first() {
        let collector = Collector::new(SamplerConfig::default());
        let first = collector.collect(&Counters::default(), 1).unwrap();
        // Busy-wait briefly so the clock and the tick counters both advance.
        let spin_until = std::time::Instant::now() + std::time::Duration::from_millis(60);
        while std::time::Instant::now() < spin_until {
            std::hint::spin_loop();
        }
        let second = collector.collect(&first.counters, 2).unwrap();

        assert!(second.sample.interval_ms.unwrap_or(0) > 0);
        let busy = second
            .sample
            .cpu
            .total_busy_percent
            .expect("a derived total");
        assert!(
            (0.0..=100.0).contains(&busy),
            "busy percent out of range: {busy}"
        );
    }

    /// The reported table is capped, but the carried tick map is not — a
    /// process outside the top-N still needs its predecessor, or it can never
    /// report a rate once it climbs the ranking.
    #[test]
    fn process_table_is_capped_but_tick_history_is_not() {
        let collector = Collector::new(SamplerConfig::default());
        let collected = collector.collect(&Counters::default(), 1).unwrap();

        let reported = collected.sample.processes.as_ref().unwrap().len();
        assert!(reported <= SamplerConfig::default().max_processes as usize);
        assert!(
            collected.counters.processes.len() >= reported,
            "tick history ({}) must cover at least the reported processes ({reported})",
            collected.counters.processes.len()
        );
    }

    /// Redaction happens inside this module, so no `ProcessSample` leaving it
    /// can carry an over-long command.
    #[test]
    fn commands_are_bounded() {
        let collector = Collector::new(SamplerConfig::default());
        let collected = collector.collect(&Counters::default(), 1).unwrap();
        for process in collected.sample.processes.iter().flatten() {
            assert!(
                process.command.chars().count() <= crate::redact::MAX_COMMAND_CHARS + 1,
                "unbounded command: {:?}",
                process.command
            );
        }
    }
}
