//! Free `&str → struct` parsers, one per `/proc` file.
//!
//! Nothing here performs I/O. That is the point: the risky logic in this crate
//! is delta derivation and redaction, and keeping the parsers pure lets both be
//! tested against checked-in fixtures — including truncated files, extra
//! columns, and counter resets — with no host access and no mocking. It is also
//! why the crate hand-rolls this instead of taking `sysinfo`, which can only be
//! handed a live host.
//!
//! Every parser returns `Option`/an empty collection for input it cannot make
//! sense of. None of them substitutes a zero.

use std::collections::BTreeMap;

use crate::types::MemorySample;

/// Aggregate CPU time from one `/proc/stat` line, in kernel ticks.
///
/// Only the two quantities a busy percentage needs are kept. Storing the raw
/// counters rather than a derived percentage is what makes a long or irregular
/// gap scale correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    /// Sum of every column on the line.
    pub total: u64,
    /// `idle + iowait`. A CPU waiting on I/O is not busy.
    pub idle: u64,
}

/// One `cpuN` line: the kernel's own index for the core, plus its counters.
///
/// The index is carried rather than implied by position. `/proc/stat` is
/// written by `for_each_online_cpu`, so an offline CPU has no line at all: with
/// cpu1 offline the second entry is cpu2, and anything that labels by position
/// would call it "core 1".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreTimes {
    /// The `N` of `cpuN`.
    pub index: u32,
    pub times: CpuTimes,
}

/// The `cpu` and `cpuN` lines of `/proc/stat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcStat {
    pub total: CpuTimes,
    /// One entry per **online** core, in file order, each tagged with its
    /// kernel index.
    pub per_core: Vec<CoreTimes>,
}

/// Lifetime byte counters for one interface, from `/proc/net/dev`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetCounters {
    pub interface: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// One row of `/proc/self/mounts`, after filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
}

/// The fields of `/proc/[pid]/stat` this crate uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStat {
    pub pid: i32,
    /// Field 2, `comm`, with its surrounding parentheses removed.
    pub name: String,
    pub utime: u64,
    pub stime: u64,
    /// Field 22. Half of a process's identity, because PIDs are reused.
    pub start_ticks: u64,
}

impl ProcessStat {
    /// Total busy ticks; the counter a CPU percentage is derived from.
    pub fn busy_ticks(&self) -> u64 {
        self.utime.saturating_add(self.stime)
    }
}

/// The fields of `/proc/[pid]/status` this crate uses. All optional: a process
/// can exit between the `stat` read and the `status` read, and kernel threads
/// have no `VmRSS` at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessStatus {
    pub uid: Option<u32>,
    pub memory_bytes: Option<u64>,
    pub thread_count: Option<u32>,
}

/// Load averages from `/proc/loadavg`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadAverage {
    pub one: f32,
    pub five: f32,
    pub fifteen: f32,
}

/// The fields of `/proc/cpuinfo` this crate uses.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuInfo {
    pub model: Option<String>,
    /// Mean `cpu MHz` across cores. Individual cores drift constantly, and no
    /// panel plots them separately.
    pub frequency_mhz: Option<u32>,
}

/// Filesystem types that describe kernel bookkeeping rather than storage.
///
/// `data-model.md` lists a representative subset; this is the full set present
/// on the fleet's hosts. Getting one wrong costs a junk row in the disks panel,
/// never a missing real filesystem — network filesystems are deliberately
/// absent from this list, because the shared root filling up is one of the main
/// things the panel exists to catch.
const PSEUDO_FS_TYPES: &[&str] = &[
    "autofs",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "debugfs",
    "devpts",
    "devtmpfs",
    "efivarfs",
    "fuse.lxcfs",
    "fusectl",
    "hugetlbfs",
    "mqueue",
    "nsfs",
    "overlay",
    "proc",
    "pstore",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "squashfs",
    "sysfs",
    "tracefs",
];

/// Parse the `cpu` and `cpuN` lines of `/proc/stat`.
///
/// Returns `None` when the aggregate `cpu` line is missing or unparsable —
/// there is no meaningful fallback, and a zeroed [`ProcStat`] would derive a
/// bogus 100% busy against the next sample.
pub fn parse_proc_stat(contents: &str) -> Option<ProcStat> {
    let mut total = None;
    let mut per_core = Vec::new();

    for line in contents.lines() {
        let Some(rest) = line.strip_prefix("cpu") else {
            // The cpu lines lead the file; everything after is intr, ctxt, and
            // friends, none of which this crate reads.
            continue;
        };
        let (index, columns) = match rest.split_once(char::is_whitespace) {
            Some((index, columns)) => (index, columns),
            None => continue,
        };
        let Some(times) = parse_cpu_times(columns) else {
            continue;
        };
        if index.is_empty() {
            total = Some(times);
        } else if index.chars().all(|c| c.is_ascii_digit())
            && let Ok(index) = index.parse::<u32>()
        {
            // The index is kept, not just validated: it is the core's identity,
            // and the file omits offline CPUs.
            per_core.push(CoreTimes { index, times });
        }
    }

    total.map(|total| ProcStat { total, per_core })
}

/// Sum a `/proc/stat` cpu line's columns.
///
/// Newer kernels append columns (`guest`, `guest_nice`, …); summing whatever is
/// present keeps `Δidle/Δtotal` correct on any kernel, where a fixed field count
/// would silently skew the ratio.
fn parse_cpu_times(columns: &str) -> Option<CpuTimes> {
    let values: Vec<u64> = columns
        .split_whitespace()
        .map(|column| column.parse::<u64>())
        .collect::<Result<_, _>>()
        .ok()?;
    // user, nice, system, idle at minimum; iowait arrived in 2.5.41.
    if values.len() < 4 {
        return None;
    }
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Some(CpuTimes {
        total: values.iter().sum(),
        idle,
    })
}

/// Parse `/proc/meminfo`.
///
/// Fields absent from the file stay `None`; a host without swap reports
/// `swap_total_bytes: None`, which the UI renders as "—" rather than as a
/// machine with zero swap that is somehow also not swapping.
pub fn parse_meminfo(contents: &str) -> MemorySample {
    let mut fields: BTreeMap<&str, u64> = BTreeMap::new();
    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let mut columns = value.split_whitespace();
        let Some(Ok(amount)) = columns.next().map(str::parse::<u64>) else {
            continue;
        };
        // Everything except `HugePages_*` is reported in kB.
        let bytes = match columns.next() {
            Some("kB") => match amount.checked_mul(1024) {
                Some(bytes) => bytes,
                None => continue,
            },
            _ => amount,
        };
        fields.insert(key, bytes);
    }

    let total_bytes = fields.get("MemTotal").copied();
    let available_bytes = fields.get("MemAvailable").copied();
    let swap_total_bytes = fields.get("SwapTotal").copied();
    let swap_free_bytes = fields.get("SwapFree").copied();

    MemorySample {
        total_bytes,
        available_bytes,
        // `total − available`, not `total − free`: MemFree excludes reclaimable
        // page cache and makes a healthy Linux box look full.
        used_bytes: subtract(total_bytes, available_bytes),
        cached_bytes: match (fields.get("Cached"), fields.get("SReclaimable")) {
            (Some(cached), reclaimable) => {
                Some(cached.saturating_add(reclaimable.copied().unwrap_or(0)))
            }
            (None, _) => None,
        },
        swap_total_bytes,
        swap_used_bytes: subtract(swap_total_bytes, swap_free_bytes),
    }
}

/// `a − b`, absent unless both sides are present. Saturating rather than
/// wrapping: `MemAvailable` can momentarily exceed `MemTotal` on some kernels,
/// and a 16-exabyte "used" figure is worse than a zero.
fn subtract(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    Some(left?.saturating_sub(right?))
}

/// Parse `/proc/loadavg`. Requires all three averages; a partial line is a
/// truncated read, not two thirds of an answer.
pub fn parse_loadavg(contents: &str) -> Option<LoadAverage> {
    let mut columns = contents.split_whitespace();
    Some(LoadAverage {
        one: columns.next()?.parse().ok()?,
        five: columns.next()?.parse().ok()?,
        fifteen: columns.next()?.parse().ok()?,
    })
}

/// Parse `/proc/uptime`, returning whole seconds since boot.
pub fn parse_uptime(contents: &str) -> Option<u64> {
    let seconds: f64 = contents.split_whitespace().next()?.parse().ok()?;
    (seconds >= 0.0).then_some(seconds as u64)
}

/// Parse `/proc/cpuinfo` for the model name and the mean current frequency.
///
/// Both fields are architecture-dependent — `model name` is absent on aarch64,
/// `cpu MHz` on hosts without cpufreq — so both are independently optional.
pub fn parse_cpuinfo(contents: &str) -> CpuInfo {
    let mut model = None;
    let mut megahertz_total = 0.0_f64;
    let mut megahertz_count = 0_u32;

    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "model name" if model.is_none() && !value.is_empty() => {
                model = Some(value.to_string());
            }
            "cpu MHz" => {
                if let Ok(megahertz) = value.parse::<f64>() {
                    megahertz_total += megahertz;
                    megahertz_count += 1;
                }
            }
            _ => {}
        }
    }

    CpuInfo {
        model,
        frequency_mhz: (megahertz_count > 0)
            .then(|| (megahertz_total / f64::from(megahertz_count)).round() as u32),
    }
}

/// Parse `/proc/net/dev`.
///
/// `lo` and interfaces that have never carried a byte are dropped here rather
/// than in the collector, so the filter is covered by the parser's own tests. A
/// down interface reappears on its own once it carries traffic.
pub fn parse_net_dev(contents: &str) -> Vec<NetCounters> {
    let mut counters = Vec::new();
    for line in contents.lines() {
        let Some((interface, columns)) = line.split_once(':') else {
            // The two header rows carry no colon.
            continue;
        };
        let interface = interface.trim();
        if interface.is_empty() || interface == "lo" {
            continue;
        }
        let values: Vec<&str> = columns.split_whitespace().collect();
        // 8 receive columns then 8 transmit columns; bytes lead each group.
        if values.len() < 9 {
            continue;
        }
        let (Ok(rx_bytes), Ok(tx_bytes)) = (values[0].parse::<u64>(), values[8].parse::<u64>())
        else {
            continue;
        };
        if rx_bytes == 0 && tx_bytes == 0 {
            continue;
        }
        counters.push(NetCounters {
            interface: interface.to_string(),
            rx_bytes,
            tx_bytes,
        });
    }
    counters
}

/// Parse `/proc/self/mounts`, keeping only mounts that describe real storage.
///
/// Pseudo filesystems are dropped by type, `tmpfs`/`ramfs` under `/run` by
/// mount point, and repeats of a device already listed by identity — the last
/// of which is what stops `/`, `/nix/store`, `/tmp`, and `/var/tmp` on one LVM
/// volume from being reported as four filesystems each claiming the whole disk.
/// The first occurrence wins, and `/proc/self/mounts` is in mount order, so
/// that is the shallowest mount point.
pub fn parse_mounts(contents: &str) -> Vec<MountEntry> {
    let mut entries = Vec::new();
    let mut seen_devices = Vec::new();

    for line in contents.lines() {
        let mut columns = line.split_whitespace();
        let (Some(device), Some(mount_point), Some(fs_type)) =
            (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        if PSEUDO_FS_TYPES.contains(&fs_type) || fs_type.starts_with("cgroup") {
            continue;
        }
        let mount_point = unescape_mount_field(mount_point);
        if matches!(fs_type, "tmpfs" | "ramfs")
            && (mount_point == "/run" || mount_point.starts_with("/run/"))
        {
            continue;
        }
        let device = unescape_mount_field(device);
        if seen_devices.contains(&device) {
            continue;
        }
        seen_devices.push(device.clone());
        entries.push(MountEntry {
            device,
            mount_point,
            fs_type: fs_type.to_string(),
        });
    }

    entries
}

/// Undo the octal escaping the kernel applies to space, tab, newline, and
/// backslash in mount fields. Without this a mount point containing a space
/// arrives as `/mnt/my\040disk` and never matches anything.
fn unescape_mount_field(field: &str) -> String {
    if !field.contains('\\') {
        return field.to_string();
    }
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let digits: String = chars.clone().take(3).collect();
        match u8::from_str_radix(&digits, 8) {
            Ok(byte) if digits.len() == 3 => {
                out.push(byte as char);
                for _ in 0..3 {
                    chars.next();
                }
            }
            // A lone backslash is legal in a device name; keep it verbatim.
            _ => out.push('\\'),
        }
    }
    out
}

/// Parse `/proc/[pid]/stat`.
///
/// `comm` is the reason this cannot be a plain `split_whitespace`: it is
/// arbitrary process-controlled text inside parentheses, so a process named
/// `(evil) R 1 1 1` would shift every subsequent field. Splitting at the *last*
/// `)` is the documented way to read this file.
pub fn parse_process_stat(contents: &str) -> Option<ProcessStat> {
    let open = contents.find('(')?;
    let close = contents.rfind(')')?;
    if close < open {
        return None;
    }
    let pid: i32 = contents[..open].trim().parse().ok()?;
    let name = contents[open + 1..close].to_string();

    // Field 3 (`state`) onwards; field N is at index N - 3.
    let fields: Vec<&str> = contents[close + 1..].split_whitespace().collect();
    let field = |number: usize| fields.get(number - 3).and_then(|v| v.parse::<u64>().ok());

    Some(ProcessStat {
        pid,
        name,
        utime: field(14)?,
        stime: field(15)?,
        start_ticks: field(22)?,
    })
}

/// Parse `/proc/[pid]/status` for owner, resident memory, and thread count.
///
/// A missing field yields `None` for that field alone — a kernel thread has no
/// `VmRSS`, and reporting it as 0 bytes would put a fabricated reading in the
/// process table.
pub fn parse_process_status(contents: &str) -> ProcessStatus {
    let mut status = ProcessStatus::default();
    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            // "real effective saved filesystem"; the real uid is the owner.
            "Uid" => status.uid = value.split_whitespace().next().and_then(|v| v.parse().ok()),
            "VmRSS" => {
                status.memory_bytes = value
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .and_then(|kib| kib.checked_mul(1024));
            }
            "Threads" => status.thread_count = value.parse().ok(),
            _ => {}
        }
    }
    status
}

/// Split the NUL-separated bytes of `/proc/[pid]/cmdline` into an argument
/// vector.
///
/// Returns `None` for an empty cmdline, which means a kernel thread — those
/// have no command line at all, and the caller falls back to `comm`.
///
/// **The elements, not a joined string.** An argument may contain spaces, and
/// joining here would destroy the only boundary information the file carries:
/// [`crate::redact::redact_argv`] would then see `--password correct horse` as
/// four tokens and mask one of them. Redaction is what joins, once it has
/// classified each argument whole.
///
/// The result is **not** redacted; [`crate::redact::redact_argv`] must run
/// before the value reaches a [`crate::types::ProcessSample`].
pub fn parse_cmdline(contents: &str) -> Option<Vec<String>> {
    let argv: Vec<String> = contents
        .split('\0')
        // Some processes rewrite argv into a single blob with trailing padding;
        // an all-whitespace element carries nothing.
        .filter(|argument| !argument.trim().is_empty())
        .map(str::to_string)
        .collect();
    (!argv.is_empty()).then_some(argv)
}

/// Parse `/etc/passwd` into a uid → name map, so a process owner can be shown
/// as a name instead of a number.
///
/// Reading the file directly rather than calling `getpwuid` keeps this pure and
/// avoids an NSS lookup that can block on a network directory service inside
/// the sampling path.
pub fn parse_passwd(contents: &str) -> BTreeMap<u32, String> {
    let mut users = BTreeMap::new();
    for line in contents.lines() {
        let mut columns = line.split(':');
        let (Some(name), Some(_password), Some(uid)) =
            (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        if let Ok(uid) = uid.parse::<u32>() {
            users.entry(uid).or_insert_with(|| name.to_string());
        }
    }
    users
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = include_str!("../tests/fixtures/stat");
    const STAT_NEXT: &str = include_str!("../tests/fixtures/stat_next");
    const MEMINFO: &str = include_str!("../tests/fixtures/meminfo");
    const LOADAVG: &str = include_str!("../tests/fixtures/loadavg");
    const UPTIME: &str = include_str!("../tests/fixtures/uptime");
    const CPUINFO: &str = include_str!("../tests/fixtures/cpuinfo");
    const NET_DEV: &str = include_str!("../tests/fixtures/net_dev");
    const NET_DEV_NEXT: &str = include_str!("../tests/fixtures/net_dev_next");
    const MOUNTS: &str = include_str!("../tests/fixtures/self_mounts");
    const PID_STAT: &str = include_str!("../tests/fixtures/pid/stat");
    const PID_STATUS: &str = include_str!("../tests/fixtures/pid/status");
    const PID_CMDLINE: &str = include_str!("../tests/fixtures/pid/cmdline");

    /// Every parser is handed a file that stops mid-line, an empty file, and a
    /// file with more columns than it expects. None may panic, and none may
    /// invent a value.
    #[test]
    fn parsers_survive_truncated_and_empty_input() {
        assert!(parse_proc_stat("").is_none());
        assert!(parse_proc_stat("cpu  1713").is_none());
        assert!(parse_proc_stat("cpu").is_none());
        assert!(parse_loadavg("").is_none());
        assert!(parse_loadavg("0.31 1.60").is_none());
        assert!(parse_uptime("").is_none());
        assert!(parse_uptime("not-a-number").is_none());
        assert!(parse_process_stat("").is_none());
        assert!(parse_process_stat("254347 (cp) R 254334").is_none());
        assert!(parse_cmdline("").is_none());
        assert!(parse_cmdline("\0\0\0").is_none());
        assert!(parse_net_dev("").is_empty());
        assert!(parse_net_dev("  eno1: 1332426109 8905402").is_empty());
        assert!(parse_mounts("").is_empty());
        assert!(parse_mounts("/dev/sda1").is_empty());
        assert_eq!(parse_cpuinfo(""), CpuInfo::default());
        assert_eq!(parse_process_status(""), ProcessStatus::default());

        let empty = parse_meminfo("");
        assert_eq!(empty.total_bytes, None);
        assert_eq!(empty.used_bytes, None);
        assert_eq!(empty.cached_bytes, None);
    }

    #[test]
    fn proc_stat_reads_aggregate_and_per_core_lines() {
        let stat = parse_proc_stat(STAT).expect("aggregate cpu line");
        assert_eq!(stat.per_core.len(), 6);
        // cpu  1713051 771 1937076 941210597 830753 582195 466133 0 0 0
        // `idle` is idle + iowait: a CPU waiting on I/O is not busy.
        assert_eq!(stat.total.idle, 941_210_597 + 830_753);
        assert_eq!(
            stat.total.total,
            1_713_051 + 771 + 1_937_076 + 941_210_597 + 830_753 + 582_195 + 466_133
        );
        // The aggregate is the sum of the cores, give or take rounding in the
        // kernel's own accounting; a wildly different figure means the line
        // split went wrong.
        let core_total: u64 = stat.per_core.iter().map(|core| core.times.total).sum();
        assert!(core_total.abs_diff(stat.total.total) < stat.total.total / 100);
        // A fully online host lists cpu0..cpu5 in order.
        let indices: Vec<u32> = stat.per_core.iter().map(|core| core.index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
    }

    /// `/proc/stat` is written by `for_each_online_cpu`, so a host with cpu1
    /// offline lists cpu0 then cpu2. The index has to be carried, not inferred
    /// from position, or the second entry is labelled "core 1".
    #[test]
    fn proc_stat_keeps_the_kernel_core_index_when_a_cpu_is_offline() {
        let stat = parse_proc_stat(
            "cpu  30 0 0 270 0\n\
             cpu0 10 0 0 90 0\n\
             cpu2 20 0 0 180 0\n\
             intr 1 2 3\n",
        )
        .unwrap();

        let indices: Vec<u32> = stat.per_core.iter().map(|core| core.index).collect();
        assert_eq!(indices, vec![0, 2], "the offline cpu1 must not be invented");
        assert_eq!(stat.per_core[1].times.total, 200);
    }

    /// Kernels keep appending columns to the cpu lines. Summing what is present
    /// keeps the ratio honest instead of quietly dropping `guest_nice`.
    #[test]
    fn proc_stat_sums_unknown_trailing_columns() {
        let four = parse_proc_stat("cpu  10 0 10 80\n").unwrap();
        assert_eq!(four.total.total, 100);
        assert_eq!(four.total.idle, 80);

        let many = parse_proc_stat("cpu  10 0 10 80 5 0 0 0 0 0 7 9\n").unwrap();
        assert_eq!(many.total.total, 121);
        assert_eq!(many.total.idle, 85);
    }

    #[test]
    fn proc_stat_ignores_non_cpu_lines() {
        let stat = parse_proc_stat(STAT).unwrap();
        let next = parse_proc_stat(STAT_NEXT).unwrap();
        // Both fixtures are real consecutive reads, so the counters must have
        // advanced; anything else means we parsed the wrong line.
        assert!(next.total.total > stat.total.total);
        assert_eq!(next.per_core.len(), stat.per_core.len());
    }

    #[test]
    fn meminfo_derives_used_from_available() {
        let memory = parse_meminfo(MEMINFO);
        assert_eq!(memory.total_bytes, Some(16_241_544 * 1024));
        assert_eq!(memory.available_bytes, Some(13_551_092 * 1024));
        assert_eq!(memory.used_bytes, Some((16_241_544 - 13_551_092) * 1024));
        assert!(memory.cached_bytes.unwrap() >= 8_280_980 * 1024);
    }

    #[test]
    fn meminfo_reports_a_missing_field_as_absent() {
        let memory = parse_meminfo("MemTotal:       16241544 kB\n");
        assert_eq!(memory.total_bytes, Some(16_241_544 * 1024));
        assert_eq!(memory.available_bytes, None);
        // Not Some(0): "we could not read MemAvailable" is not "nothing is
        // used".
        assert_eq!(memory.used_bytes, None);
        assert_eq!(memory.swap_used_bytes, None);
    }

    #[test]
    fn meminfo_tolerates_extra_columns_and_junk_lines() {
        let memory = parse_meminfo(
            "MemTotal:       1024 kB extra column\n\
             garbage without a colon\n\
             MemAvailable:    512 kB\n\
             Cached:          128 kB\n\
             SReclaimable:     64 kB\n\
             HugePages_Total:   0\n",
        );
        assert_eq!(memory.total_bytes, Some(1024 * 1024));
        assert_eq!(memory.used_bytes, Some(512 * 1024));
        assert_eq!(memory.cached_bytes, Some(192 * 1024));
    }

    #[test]
    fn loadavg_reads_three_averages() {
        let load = parse_loadavg(LOADAVG).expect("load averages");
        assert_eq!(load.one, 0.31);
        assert_eq!(load.five, 1.60);
        assert_eq!(load.fifteen, 1.02);
    }

    #[test]
    fn uptime_truncates_to_whole_seconds() {
        assert_eq!(parse_uptime(UPTIME), Some(1_579_057));
        assert_eq!(parse_uptime("12.99 3.00\n"), Some(12));
        assert_eq!(parse_uptime("-1.0 0.0\n"), None);
    }

    #[test]
    fn cpuinfo_reads_model_and_mean_frequency() {
        let info = parse_cpuinfo(CPUINFO);
        assert_eq!(
            info.model.as_deref(),
            Some("Intel(R) Core(TM) i5-8500T CPU @ 2.10GHz")
        );
        let frequency = info.frequency_mhz.expect("mean frequency");
        assert!((400..=5000).contains(&frequency), "implausible {frequency}");
    }

    #[test]
    fn cpuinfo_fields_are_independently_optional() {
        let no_frequency = parse_cpuinfo("model name\t: Cortex-A72\nprocessor\t: 0\n");
        assert_eq!(no_frequency.model.as_deref(), Some("Cortex-A72"));
        assert_eq!(no_frequency.frequency_mhz, None);

        let no_model = parse_cpuinfo("cpu MHz\t\t: 1000.0\ncpu MHz\t\t: 2000.0\n");
        assert_eq!(no_model.model, None);
        assert_eq!(no_model.frequency_mhz, Some(1500));
    }

    #[test]
    fn net_dev_reads_lifetime_counters() {
        let counters = parse_net_dev(NET_DEV);
        let eno1 = counters
            .iter()
            .find(|entry| entry.interface == "eno1")
            .expect("eno1");
        assert_eq!(eno1.rx_bytes, 1_332_426_109);
        assert_eq!(eno1.tx_bytes, 20_562_346);

        // Real consecutive reads: at least one interface must have moved.
        let next = parse_net_dev(NET_DEV_NEXT);
        assert!(
            next.iter()
                .zip(counters.iter())
                .any(|(after, before)| after.rx_bytes > before.rx_bytes
                    || after.tx_bytes > before.tx_bytes)
        );
    }

    #[test]
    fn net_dev_skips_loopback_and_silent_interfaces() {
        let counters = parse_net_dev(NET_DEV);
        assert!(counters.iter().all(|entry| entry.interface != "lo"));
        // incusbr0 is up but has never carried a byte in either direction.
        assert!(counters.iter().all(|entry| entry.interface != "incusbr0"));
        // tailscale0 has 86 rx bytes, which is "used", not "silent".
        assert!(counters.iter().any(|entry| entry.interface == "tailscale0"));
    }

    #[test]
    fn net_dev_tolerates_extra_columns() {
        let counters = parse_net_dev(
            "Inter-|   Receive                                                |  Transmit\n\
             eth0: 100 1 0 0 0 0 0 0 200 2 0 0 0 0 0 0 999 999\n",
        );
        assert_eq!(counters.len(), 1);
        assert_eq!(counters[0].rx_bytes, 100);
        assert_eq!(counters[0].tx_bytes, 200);
    }

    #[test]
    fn mounts_keep_real_storage_and_drop_pseudo_filesystems() {
        let mounts = parse_mounts(MOUNTS);
        let point = |path: &str| mounts.iter().find(|entry| entry.mount_point == path);

        let root = point("/").expect("root filesystem");
        assert_eq!(root.fs_type, "ext4");
        assert!(point("/boot").is_some());
        assert!(
            mounts
                .iter()
                .all(|entry| !PSEUDO_FS_TYPES.contains(&entry.fs_type.as_str()))
        );
        assert!(point("/proc").is_none());
        assert!(point("/sys/fs/cgroup").is_none());
        assert!(point("/run").is_none());
        assert!(point("/run/keys").is_none());
    }

    /// The shared root filling up is one of the main things the disks panel
    /// exists to catch, so an NFS mount must survive the filter.
    #[test]
    fn mounts_keep_nfs() {
        let mounts = parse_mounts(MOUNTS);
        let shared = mounts
            .iter()
            .find(|entry| entry.mount_point == "/srv/vibe-kanban-shared")
            .expect("shared NFS mount");
        assert_eq!(shared.fs_type, "nfs");
        assert_eq!(shared.device, "172.16.0.99:/var/nfs/shared/VibeKanban");
    }

    #[test]
    fn mounts_drop_repeats_of_a_device_already_listed() {
        let mounts = parse_mounts(MOUNTS);
        // /, /nix/store, /tmp, and /var/tmp are one LVM volume.
        let root_device_mounts = mounts
            .iter()
            .filter(|entry| entry.device == "/dev/mapper/pool-root")
            .count();
        assert_eq!(root_device_mounts, 1);
        assert!(mounts.iter().all(|entry| entry.mount_point != "/nix/store"));
    }

    #[test]
    fn mounts_unescape_octal_fields() {
        let mounts = parse_mounts("/dev/sdb1 /mnt/my\\040disk ext4 rw 0 0\n");
        assert_eq!(mounts[0].mount_point, "/mnt/my disk");

        // A backslash that does not introduce a three-digit octal escape is
        // part of the name.
        let odd = parse_mounts("/dev/sdb1 /mnt/back\\slash ext4 rw 0 0\n");
        assert_eq!(odd[0].mount_point, "/mnt/back\\slash");
    }

    #[test]
    fn process_stat_reads_identity_and_cpu_ticks() {
        let stat = parse_process_stat(PID_STAT).expect("process stat");
        assert_eq!(stat.pid, 254_347);
        assert_eq!(stat.name, "cp");
        assert_eq!(stat.start_ticks, 157_905_759);
        assert_eq!(stat.busy_ticks(), stat.utime + stat.stime);
    }

    /// `comm` is process-controlled. A process that names itself
    /// `evil) R 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1` would shift every
    /// field if the parser split on the *first* `)`.
    #[test]
    fn process_stat_handles_a_hostile_comm() {
        let hostile = "42 (evil) R 0 0 0 0 0 0) X (weird name) R 1 2 3 4 5 6 7 8 9 10 \
                       111 222 13 14 15 16 17 18 999 20 21 22";
        let stat = parse_process_stat(hostile).expect("hostile comm");
        assert_eq!(stat.pid, 42);
        assert_eq!(stat.name, "evil) R 0 0 0 0 0 0) X (weird name");
        assert_eq!(stat.utime, 111);
        assert_eq!(stat.stime, 222);
        assert_eq!(stat.start_ticks, 999);
    }

    #[test]
    fn process_status_reads_owner_memory_and_threads() {
        let status = parse_process_status(PID_STATUS);
        assert_eq!(status.uid, Some(994));
        assert_eq!(status.memory_bytes, Some(3108 * 1024));
        assert_eq!(status.thread_count, Some(1));
    }

    #[test]
    fn process_status_fields_are_independently_optional() {
        // A kernel thread: no VmRSS at all.
        let status = parse_process_status("Name:\tkthreadd\nUid:\t0\t0\t0\t0\nThreads:\t1\n");
        assert_eq!(status.uid, Some(0));
        assert_eq!(status.memory_bytes, None);
        assert_eq!(status.thread_count, Some(1));
    }

    #[test]
    fn cmdline_splits_nul_separated_arguments() {
        assert_eq!(
            parse_cmdline(PID_CMDLINE),
            Some(vec![
                "cp".to_string(),
                "/proc/self/cmdline".to_string(),
                "crates/node-metrics/tests/fixtures/pid/cmdline".to_string(),
            ])
        );
        assert_eq!(
            parse_cmdline("a\0b\0\0c\0"),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
        // argv rewritten into one blob, as postgres and nginx do.
        assert_eq!(
            parse_cmdline("postgres: writer process   \0"),
            Some(vec!["postgres: writer process   ".to_string()])
        );
    }

    /// The boundary is the whole point of returning a vector: an argument
    /// containing spaces is **one** element, so redaction can mask it whole.
    /// Joining here is what let a multi-word secret through a word at a time.
    #[test]
    fn cmdline_keeps_arguments_containing_spaces_whole() {
        let argv = parse_cmdline("app\0--password\0correct horse battery\0").unwrap();
        assert_eq!(
            argv,
            vec![
                "app".to_string(),
                "--password".to_string(),
                "correct horse battery".to_string(),
            ]
        );
    }

    #[test]
    fn passwd_maps_uid_to_name() {
        let users = parse_passwd(
            "root:x:0:0:root:/root:/bin/bash\n\
             daemon:x:1:1::/:/sbin/nologin\n\
             malformed-line\n\
             vibe-kanban:x:994:990::/var/lib/vk:/bin/sh\n",
        );
        assert_eq!(users.get(&0).map(String::as_str), Some("root"));
        assert_eq!(users.get(&994).map(String::as_str), Some("vibe-kanban"));
        assert_eq!(users.get(&12345), None);
    }
}
