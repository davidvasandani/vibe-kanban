//! Keep the worker's control plane out of its children's memory budget.
//!
//! Agents, pollers and builds used to share the worker's single leaf cgroup.
//! A memory ceiling there throttles *every* task in the cgroup: on think5 a
//! 5 GB `go vet` held the unit at `memory.high`, the worker's accept loop and
//! heartbeats stalled in reclaim, and the coordinator wrote the host off.
//!
//! With systemd delegation (`Delegate=memory`, `DelegateSubgroup=supervisor`)
//! the worker starts in `<unit>/supervisor`. This module creates the sibling
//! `<unit>/<jobs>` with the configured limits and continuously moves every
//! process other than the worker itself into it. Children spawned through the
//! shared helpers (`utils::command_ext`: every executor, plus worker scripts)
//! join `jobs` between fork and exec, so they are charged to it from their
//! first page. The sweep is the backstop for every other path (PTYs, one-off
//! commands): cgroup v2 does not migrate existing charges, so what such a
//! child allocates before its sweep stays charged to the unlimited supervisor.

use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use serde::Serialize;
use tokio_util::sync::CancellationToken;

pub const WORKER_JOB_CGROUP_ENV: &str = "VK_WORKER_JOB_CGROUP";
pub const WORKER_JOB_MEMORY_HIGH_ENV: &str = "VK_WORKER_JOB_MEMORY_HIGH";
pub const WORKER_JOB_MEMORY_MAX_ENV: &str = "VK_WORKER_JOB_MEMORY_MAX";

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const SWEEP_INTERVAL: Duration = Duration::from_millis(200);

/// A cgroup v2 memory limit as configured: `max`, bytes (optionally with a
/// K/M/G/T suffix), or a percentage of host RAM resolved at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLimit {
    Unlimited,
    Bytes(u64),
    PercentOfRam(u8),
}

impl MemoryLimit {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("max") {
            return Some(Self::Unlimited);
        }
        if let Some(percent) = value.strip_suffix('%') {
            return percent
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|percent| (1..=100).contains(percent))
                .map(Self::PercentOfRam);
        }
        let (digits, multiplier) = match value.char_indices().last()? {
            (index, 'K' | 'k') => (&value[..index], 1u64 << 10),
            (index, 'M' | 'm') => (&value[..index], 1 << 20),
            (index, 'G' | 'g') => (&value[..index], 1 << 30),
            (index, 'T' | 't') => (&value[..index], 1 << 40),
            _ => (value, 1),
        };
        digits
            .parse::<u64>()
            .ok()
            .filter(|bytes| *bytes > 0)
            .and_then(|bytes| bytes.checked_mul(multiplier))
            .map(Self::Bytes)
    }

    /// The value written to `memory.high` / `memory.max`.
    fn resolve(self, mem_total_bytes: u64) -> String {
        match self {
            Self::Unlimited => "max".into(),
            Self::Bytes(bytes) => bytes.to_string(),
            Self::PercentOfRam(percent) => {
                (u128::from(mem_total_bytes) * u128::from(percent) / 100).to_string()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobCgroupConfig {
    /// Name of the sibling cgroup that receives child processes.
    pub name: String,
    pub memory_high: MemoryLimit,
    pub memory_max: MemoryLimit,
}

impl JobCgroupConfig {
    /// A cgroup name, not a path: it must stay a direct sibling of the
    /// supervisor inside the delegated subtree.
    pub fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && !name.starts_with('.')
            && !name.starts_with("cgroup.")
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    }
}

/// Reported on `/health` so a worker running without isolation is visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobCgroupStatus {
    Disabled,
    Isolated {
        cgroup: String,
        memory_high: String,
        memory_max: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug)]
struct JobCgroup {
    supervisor: PathBuf,
    jobs: PathBuf,
}

impl JobCgroup {
    /// Create the jobs cgroup beside `own` (the worker's current cgroup) and
    /// apply its limits. Idempotent across worker restarts.
    fn establish(own: &Path, config: &JobCgroupConfig, mem_total_bytes: u64) -> io::Result<Self> {
        let parent = own.parent().ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidInput, "worker is in the root cgroup")
        })?;
        if own.file_name().and_then(|name| name.to_str()) == Some(config.name.as_str()) {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "worker already runs inside the jobs cgroup; set DelegateSubgroup to a different name",
            ));
        }
        let controllers = fs::read_to_string(parent.join("cgroup.controllers"))?;
        if !controllers.split_whitespace().any(|c| c == "memory") {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                format!(
                    "memory controller is not delegated to {} (needs Delegate=memory)",
                    parent.display()
                ),
            ));
        }
        let subtree = fs::read_to_string(parent.join("cgroup.subtree_control"))?;
        if !subtree.split_whitespace().any(|c| c == "memory") {
            // Fails with EBUSY if the parent still holds processes, i.e. the
            // unit lacks DelegateSubgroup and the worker sits in its root.
            fs::write(parent.join("cgroup.subtree_control"), "+memory")?;
        }
        let jobs = parent.join(&config.name);
        match fs::create_dir(&jobs) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        fs::write(
            jobs.join("memory.high"),
            config.memory_high.resolve(mem_total_bytes),
        )?;
        fs::write(
            jobs.join("memory.max"),
            config.memory_max.resolve(mem_total_bytes),
        )?;
        Ok(Self {
            supervisor: own.to_path_buf(),
            jobs,
        })
    }

    /// Move every process except `self_pid` from the supervisor cgroup into
    /// the jobs cgroup. Returns how many moved, or the first migration error
    /// after attempting every process.
    fn sweep(&self, self_pid: u32) -> io::Result<usize> {
        let procs = fs::read_to_string(self.supervisor.join("cgroup.procs"))?;
        let mut moved = 0;
        let mut first_error = None;
        for pid in procs
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
        {
            if pid == self_pid {
                continue;
            }
            match fs::write(self.jobs.join("cgroup.procs"), pid.to_string()) {
                Ok(()) => moved += 1,
                // Exited between the read and the write.
                Err(error) if error.raw_os_error() == Some(3 /* ESRCH */) => {}
                Err(error) => {
                    first_error.get_or_insert(io::Error::new(
                        error.kind(),
                        format!("moving pid {pid} into {}: {error}", self.jobs.display()),
                    ));
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(moved),
        }
    }
}

/// The worker's own cgroup path relative to the cgroup2 mount, from the
/// unified (`0::`) line of `/proc/self/cgroup`.
fn parse_proc_self_cgroup(contents: &str) -> Option<&str> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::trim)
        .filter(|path| path.starts_with('/'))
}

fn parse_mem_total_bytes(meminfo: &str) -> Option<u64> {
    meminfo.lines().find_map(|line| {
        let kib = line.strip_prefix("MemTotal:")?.trim().strip_suffix("kB")?;
        kib.trim().parse::<u64>().ok()?.checked_mul(1024)
    })
}

/// Live isolation state for `/health`. The sweeper flips it to `Failed` while
/// migrations fail and back once they succeed, so health never claims an
/// isolation that is not being enforced.
#[derive(Debug, Clone)]
pub struct JobCgroupHandle(Arc<RwLock<JobCgroupStatus>>);

impl JobCgroupHandle {
    fn new(status: JobCgroupStatus) -> Self {
        Self(Arc::new(RwLock::new(status)))
    }

    pub fn status(&self) -> JobCgroupStatus {
        self.0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set(&self, status: JobCgroupStatus) {
        *self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    }
}

/// Establish isolation, register the jobs cgroup for pre-exec placement, and
/// start the sweeper. Never fails the worker: a worker without isolation still
/// serves, and says so on `/health`.
pub fn start(config: Option<&JobCgroupConfig>, shutdown: CancellationToken) -> JobCgroupHandle {
    let Some(config) = config else {
        return JobCgroupHandle::new(JobCgroupStatus::Disabled);
    };
    let (cgroup, isolated) = match establish_from_host(config) {
        Ok(established) => established,
        Err(error) => return JobCgroupHandle::new(failed(error)),
    };
    // Children spawned through the shared helpers join `jobs` before exec,
    // so their memory is never charged to the supervisor; the sweeper covers
    // every other spawn path.
    #[cfg(unix)]
    if !utils::command_ext::set_spawn_cgroup(&cgroup.jobs.join("cgroup.procs")) {
        tracing::warn!("spawn cgroup already registered; relying on the sweeper alone");
    }
    let handle = JobCgroupHandle::new(isolated.clone());
    match spawn_sweeper(cgroup, isolated.clone(), handle.clone(), shutdown) {
        Ok(()) => {
            tracing::info!(status = ?isolated, "worker children isolated from the control plane")
        }
        Err(error) => handle.set(failed(error)),
    }
    handle
}

fn failed(error: io::Error) -> JobCgroupStatus {
    tracing::error!(
        %error,
        "worker children are not isolated from the control plane; a child's memory pressure can stall this worker"
    );
    JobCgroupStatus::Failed {
        error: error.to_string(),
    }
}

fn establish_from_host(config: &JobCgroupConfig) -> io::Result<(JobCgroup, JobCgroupStatus)> {
    let proc_self = fs::read_to_string("/proc/self/cgroup")?;
    let relative = parse_proc_self_cgroup(&proc_self).ok_or_else(|| {
        io::Error::new(
            ErrorKind::Unsupported,
            "no cgroup v2 entry in /proc/self/cgroup",
        )
    })?;
    let own = Path::new(CGROUP_ROOT).join(relative.trim_start_matches('/'));
    let mem_total = parse_mem_total_bytes(&fs::read_to_string("/proc/meminfo")?)
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "MemTotal missing"))?;
    let cgroup = JobCgroup::establish(&own, config, mem_total)?;
    cgroup.sweep(std::process::id())?;
    let status = JobCgroupStatus::Isolated {
        cgroup: cgroup.jobs.display().to_string(),
        memory_high: config.memory_high.resolve(mem_total),
        memory_max: config.memory_max.resolve(mem_total),
    };
    Ok((cgroup, status))
}

/// A plain thread, not a tokio task: it must keep moving children even when
/// the runtime is saturated, which is exactly when a runaway child appears.
fn spawn_sweeper(
    cgroup: JobCgroup,
    isolated: JobCgroupStatus,
    handle: JobCgroupHandle,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let self_pid = std::process::id();
    thread::Builder::new()
        .name("job-cgroup-sweeper".into())
        .spawn(move || {
            let mut failing = false;
            while !shutdown.is_cancelled() {
                match cgroup.sweep(self_pid) {
                    Ok(_) if failing => {
                        failing = false;
                        tracing::info!("job cgroup sweep recovered");
                        handle.set(isolated.clone());
                    }
                    Ok(_) => {}
                    Err(error) if !failing => {
                        failing = true;
                        handle.set(failed(error));
                    }
                    Err(_) => {}
                }
                thread::sleep(SWEEP_INTERVAL);
            }
        })
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_unit(controllers: &str) -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let unit = root.path().join("vibe-kanban-worker.service");
        let supervisor = unit.join("supervisor");
        fs::create_dir_all(&supervisor).unwrap();
        fs::write(unit.join("cgroup.controllers"), controllers).unwrap();
        fs::write(unit.join("cgroup.subtree_control"), "").unwrap();
        (root, supervisor)
    }

    fn config() -> JobCgroupConfig {
        JobCgroupConfig {
            name: "jobs".into(),
            memory_high: MemoryLimit::PercentOfRam(60),
            memory_max: MemoryLimit::Bytes(12 << 30),
        }
    }

    #[test]
    fn parses_memory_limits() {
        assert_eq!(MemoryLimit::parse("max"), Some(MemoryLimit::Unlimited));
        assert_eq!(
            MemoryLimit::parse("60%"),
            Some(MemoryLimit::PercentOfRam(60))
        );
        assert_eq!(MemoryLimit::parse("8G"), Some(MemoryLimit::Bytes(8 << 30)));
        assert_eq!(
            MemoryLimit::parse("512M"),
            Some(MemoryLimit::Bytes(512 << 20))
        );
        assert_eq!(
            MemoryLimit::parse("1048576"),
            Some(MemoryLimit::Bytes(1 << 20))
        );
        for invalid in ["", "0", "0%", "101%", "-1", "8GB", "G", "abc", "1.5G"] {
            assert_eq!(MemoryLimit::parse(invalid), None, "{invalid:?}");
        }
        assert_eq!(
            MemoryLimit::PercentOfRam(50).resolve(16 << 30),
            (8u64 << 30).to_string()
        );
        assert_eq!(MemoryLimit::Unlimited.resolve(16 << 30), "max");
    }

    #[test]
    fn job_cgroup_names_stay_direct_siblings() {
        assert!(JobCgroupConfig::valid_name("jobs"));
        assert!(JobCgroupConfig::valid_name("agent-jobs_1"));
        for invalid in ["", "../jobs", "a/b", ".control", "cgroup.procs", "jobs "] {
            assert!(!JobCgroupConfig::valid_name(invalid), "{invalid:?}");
        }
    }

    #[test]
    fn reads_unified_cgroup_and_mem_total() {
        let contents = "12:pids:/legacy\n0::/system.slice/vibe-kanban-worker.service/supervisor\n";
        assert_eq!(
            parse_proc_self_cgroup(contents),
            Some("/system.slice/vibe-kanban-worker.service/supervisor")
        );
        assert_eq!(parse_proc_self_cgroup("1:name=systemd:/x\n"), None);
        assert_eq!(
            parse_mem_total_bytes("MemTotal:       16252928 kB\nMemFree: 1 kB\n"),
            Some(16_252_928 * 1024)
        );
    }

    #[test]
    fn establishes_limited_sibling_and_enables_memory_controller() {
        let (_root, supervisor) = fake_unit("cpu memory pids");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 20 << 30).unwrap();

        let unit = supervisor.parent().unwrap();
        assert_eq!(cgroup.jobs, unit.join("jobs"));
        assert_eq!(
            fs::read_to_string(unit.join("cgroup.subtree_control")).unwrap(),
            "+memory"
        );
        assert_eq!(
            fs::read_to_string(unit.join("jobs/memory.high")).unwrap(),
            ((20u64 << 30) * 60 / 100).to_string()
        );
        assert_eq!(
            fs::read_to_string(unit.join("jobs/memory.max")).unwrap(),
            (12u64 << 30).to_string()
        );
        // A restarted worker finds its jobs cgroup already present.
        JobCgroup::establish(&supervisor, &config(), 20 << 30).unwrap();
    }

    #[test]
    fn refuses_without_delegated_memory_or_when_already_inside_jobs() {
        let (_root, supervisor) = fake_unit("cpu pids");
        let error = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap_err();
        assert!(error.to_string().contains("Delegate=memory"), "{error}");

        let (_root, supervisor) = fake_unit("memory");
        let jobs = supervisor.parent().unwrap().join("jobs");
        fs::create_dir(&jobs).unwrap();
        assert!(JobCgroup::establish(&jobs, &config(), 1 << 30).is_err());
    }

    #[test]
    fn sweep_moves_every_process_but_the_worker() {
        let (_root, supervisor) = fake_unit("memory");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap();
        fs::write(supervisor.join("cgroup.procs"), "100\n200\n300\n").unwrap();

        // A plain file keeps only the last write; the kernel file appends a
        // migration per write. The count is what proves the worker stayed.
        let jobs_procs = cgroup.jobs.join("cgroup.procs");
        assert_eq!(cgroup.sweep(200).unwrap(), 2);
        assert_eq!(fs::read_to_string(&jobs_procs).unwrap(), "300");
    }

    #[test]
    fn sweep_reports_migration_failures_instead_of_claiming_isolation() {
        let (_root, supervisor) = fake_unit("memory");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap();
        fs::write(supervisor.join("cgroup.procs"), "100\n200\n").unwrap();
        // A directory where `cgroup.procs` should be makes every write fail.
        fs::create_dir(cgroup.jobs.join("cgroup.procs")).unwrap();

        let error = cgroup.sweep(200).unwrap_err();
        assert!(error.to_string().contains("moving pid 100"), "{error}");

        let handle = JobCgroupHandle::new(JobCgroupStatus::Disabled);
        handle.set(failed(error));
        assert!(matches!(handle.status(), JobCgroupStatus::Failed { .. }));
    }
}
