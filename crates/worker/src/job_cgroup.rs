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
//!
//! One aggregate budget still let a single runaway build (think3, 2026-10-15:
//! a `go build` at ~50% of RAM on a swapless node) push every co-resident
//! agent into reclaim. So `jobs` is now a parent: children land in the
//! default leaf `jobs/shared`, and each registered execution gets its own
//! `jobs/exec-<id>` with per-execution limits. The sweeper moves an
//! execution's process group, and any descendant that left it, out of
//! `shared` into that cgroup. A runaway build is then throttled and OOM-killed
//! inside its own execution, well before the aggregate fills.

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const WORKER_JOB_CGROUP_ENV: &str = "VK_WORKER_JOB_CGROUP";
pub const WORKER_JOB_MEMORY_HIGH_ENV: &str = "VK_WORKER_JOB_MEMORY_HIGH";
pub const WORKER_JOB_MEMORY_MAX_ENV: &str = "VK_WORKER_JOB_MEMORY_MAX";
pub const WORKER_EXECUTION_MEMORY_HIGH_ENV: &str = "VK_WORKER_EXECUTION_MEMORY_HIGH";
pub const WORKER_EXECUTION_MEMORY_MAX_ENV: &str = "VK_WORKER_EXECUTION_MEMORY_MAX";
pub const WORKER_JOB_CPU_WEIGHT_ENV: &str = "VK_WORKER_JOB_CPU_WEIGHT";
pub const WORKER_JOB_IO_WEIGHT_ENV: &str = "VK_WORKER_JOB_IO_WEIGHT";

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const SWEEP_INTERVAL: Duration = Duration::from_millis(200);
/// Default leaf for children that belong to no registered execution. cgroup
/// v2 forbids processes in `jobs` itself once its children have controllers.
const SHARED_LEAF: &str = "shared";
const EXECUTION_PREFIX: &str = "exec-";
/// Bound on the parent walk when attributing a process to an execution.
const MAX_ANCESTRY_HOPS: usize = 32;

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
    /// Limits of each execution's own cgroup inside `jobs`.
    pub execution_memory_high: MemoryLimit,
    pub execution_memory_max: MemoryLimit,
    /// `cpu.weight` / `io.weight` of `jobs`, relative to the supervisor's
    /// default 100. Best effort: applied only where the controller exists.
    pub cpu_weight: Option<u16>,
    pub io_weight: Option<u16>,
}

impl JobCgroupConfig {
    /// A cgroup v2 weight is `1..=10000`.
    pub fn parse_weight(value: &str) -> Option<u16> {
        value
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=10_000).contains(weight))
    }

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
        execution_memory_high: String,
        execution_memory_max: String,
        weights: WeightStatus,
    },
    Failed {
        error: String,
    },
}

/// What happened to each configured weight. `io.weight` is honoured only by
/// an IO controller that implements it (BFQ or iocost); with the `none`
/// scheduler it is accepted but advisory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeightStatus {
    pub cpu: String,
    pub io: String,
}

/// Executions whose processes the sweeper places, keyed by execution id.
#[derive(Debug, Default)]
struct Executions {
    /// Process-group leader of each live execution.
    leaders: HashMap<Uuid, u32>,
    /// Ended executions whose cgroup is removed once it is empty.
    released: HashSet<Uuid>,
}

/// `(ppid, pgid)` of a process. A trait so tests can supply a process table.
trait ProcessTable {
    fn parent_and_group(&self, pid: u32) -> Option<(u32, u32)>;
}

struct HostProcesses;

impl ProcessTable for HostProcesses {
    fn parent_and_group(&self, pid: u32) -> Option<(u32, u32)> {
        parse_parent_and_group(&fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
    }
}

/// `ppid` and `pgrp` from `/proc/<pid>/stat`. The command name may contain
/// spaces and parentheses, so fields are counted from the last `)`.
fn parse_parent_and_group(stat: &str) -> Option<(u32, u32)> {
    let mut fields = stat.get(stat.rfind(')')? + 1..)?.split_whitespace();
    let _state = fields.next()?;
    let ppid = fields.next()?.parse().ok()?;
    let pgid = fields.next()?.parse().ok()?;
    Some((ppid, pgid))
}

#[derive(Debug)]
struct JobCgroup {
    supervisor: PathBuf,
    jobs: PathBuf,
    shared: PathBuf,
    execution_memory_high: String,
    execution_memory_max: String,
    weights: WeightStatus,
    executions: Mutex<Executions>,
    /// `fs::remove_dir` on cgroupfs; tests substitute a recursive removal
    /// because a plain directory with interface files is not removable.
    remove_dir: fn(&Path) -> io::Result<()>,
}

fn read_pids(procs: &Path) -> io::Result<Vec<u32>> {
    match fs::read_to_string(procs) {
        Ok(contents) => Ok(contents
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .collect()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Move `pid` into the cgroup at `target`. A process that exited between the
/// read and the write is not an error.
fn move_pid(target: &Path, pid: u32) -> io::Result<bool> {
    match fs::write(target.join("cgroup.procs"), pid.to_string()) {
        Ok(()) => Ok(true),
        Err(error) if error.raw_os_error() == Some(3 /* ESRCH */) => Ok(false),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("moving pid {pid} into {}: {error}", target.display()),
        )),
    }
}

fn controller_listed(path: &Path, controller: &str) -> bool {
    fs::read_to_string(path)
        .is_ok_and(|list| list.split_whitespace().any(|listed| listed == controller))
}

/// Enable `controller` for `parent`'s children and write `weight` into
/// `child`. Never fails the worker; the outcome is reported on `/health`.
fn apply_weight(parent: &Path, child: &Path, controller: &str, weight: Option<u16>) -> String {
    let Some(weight) = weight else {
        return "unset".into();
    };
    if !controller_listed(&parent.join("cgroup.controllers"), controller) {
        return format!("unavailable: {controller} controller is not available to the unit");
    }
    let enable = || -> io::Result<()> {
        if !controller_listed(&parent.join("cgroup.subtree_control"), controller) {
            fs::write(
                parent.join("cgroup.subtree_control"),
                format!("+{controller}"),
            )?;
        }
        fs::write(
            child.join(format!("{controller}.weight")),
            weight.to_string(),
        )
    };
    match enable() {
        Ok(()) => format!("applied {weight}"),
        Err(error) => {
            tracing::warn!(%error, controller, "job cgroup weight not applied");
            format!("unavailable: {error}")
        }
    }
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
        create_dir_idempotent(&jobs)?;
        fs::write(
            jobs.join("memory.high"),
            config.memory_high.resolve(mem_total_bytes),
        )?;
        fs::write(
            jobs.join("memory.max"),
            config.memory_max.resolve(mem_total_bytes),
        )?;
        let weights = WeightStatus {
            cpu: apply_weight(parent, &jobs, "cpu", config.cpu_weight),
            io: apply_weight(parent, &jobs, "io", config.io_weight),
        };
        let shared = jobs.join(SHARED_LEAF);
        create_dir_idempotent(&shared)?;
        // A jobs cgroup left populated by the previous layout must be emptied
        // before its children can have controllers (no internal processes).
        for pid in read_pids(&jobs.join("cgroup.procs"))? {
            move_pid(&shared, pid)?;
        }
        if !controller_listed(&jobs.join("cgroup.subtree_control"), "memory") {
            fs::write(jobs.join("cgroup.subtree_control"), "+memory")?;
        }
        Ok(Self {
            supervisor: own.to_path_buf(),
            jobs,
            shared,
            execution_memory_high: config.execution_memory_high.resolve(mem_total_bytes),
            execution_memory_max: config.execution_memory_max.resolve(mem_total_bytes),
            weights,
            executions: Mutex::new(Executions::default()),
            remove_dir: |path| fs::remove_dir(path),
        })
    }

    fn execution_path(&self, execution_id: Uuid) -> PathBuf {
        self.jobs.join(format!("{EXECUTION_PREFIX}{execution_id}"))
    }

    fn executions(&self) -> std::sync::MutexGuard<'_, Executions> {
        self.executions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Give `execution_id` its own limited cgroup and move its process group
    /// leader there at once; the sweeper follows with the rest of the group.
    fn register_execution(&self, execution_id: Uuid, leader_pid: u32) -> io::Result<()> {
        let path = self.execution_path(execution_id);
        create_dir_idempotent(&path)?;
        fs::write(path.join("memory.high"), &self.execution_memory_high)?;
        fs::write(path.join("memory.max"), &self.execution_memory_max)?;
        {
            let mut executions = self.executions();
            executions.released.remove(&execution_id);
            executions.leaders.insert(execution_id, leader_pid);
        }
        move_pid(&path, leader_pid)?;
        Ok(())
    }

    /// Stop placing processes for `execution_id`; its cgroup is removed once
    /// empty. Processes still in it keep running under its limits.
    fn release_execution(&self, execution_id: Uuid) {
        let mut executions = self.executions();
        if executions.leaders.remove(&execution_id).is_some() {
            executions.released.insert(execution_id);
        }
    }

    /// SIGKILL every process in the execution's cgroup, including descendants
    /// that left its process group. A D-state process dies once its I/O ends.
    fn kill_execution(&self, execution_id: Uuid) -> io::Result<()> {
        let path = self.execution_path(execution_id);
        if !path.exists() {
            return Ok(());
        }
        fs::write(path.join("cgroup.kill"), "1")
    }

    /// Move processes in `shared` that belong to a registered execution into
    /// its cgroup: a process belongs to the execution whose leader is the
    /// process itself, its process group, or an ancestor, or whose cgroup
    /// already holds an ancestor (a descendant that called `setsid`).
    fn place_executions(&self, table: &impl ProcessTable) -> io::Result<usize> {
        let leaders = self.executions().leaders.clone();
        if leaders.is_empty() {
            return Ok(0);
        }
        let by_leader: HashMap<u32, Uuid> = leaders.iter().map(|(id, pid)| (*pid, *id)).collect();
        let mut members: HashMap<u32, Uuid> = HashMap::new();
        for execution_id in leaders.keys() {
            for pid in read_pids(&self.execution_path(*execution_id).join("cgroup.procs"))? {
                members.insert(pid, *execution_id);
            }
        }
        let mut moved = 0;
        for pid in read_pids(&self.shared.join("cgroup.procs"))? {
            let mut current = pid;
            let mut owner = None;
            for _ in 0..MAX_ANCESTRY_HOPS {
                if let Some(id) = by_leader.get(&current).or_else(|| members.get(&current)) {
                    owner = Some(*id);
                    break;
                }
                let Some((ppid, pgid)) = table.parent_and_group(current) else {
                    break;
                };
                if let Some(id) = by_leader.get(&pgid) {
                    owner = Some(*id);
                    break;
                }
                if ppid <= 1 {
                    break;
                }
                current = ppid;
            }
            if let Some(execution_id) = owner
                && move_pid(&self.execution_path(execution_id), pid)?
            {
                members.insert(pid, execution_id);
                moved += 1;
            }
        }
        Ok(moved)
    }

    /// Remove released execution cgroups that no longer hold processes.
    fn remove_released(&self) {
        let released: Vec<Uuid> = self.executions().released.iter().copied().collect();
        for execution_id in released {
            let path = self.execution_path(execution_id);
            let empty = read_pids(&path.join("cgroup.procs")).is_ok_and(|pids| pids.is_empty());
            if !empty {
                continue;
            }
            match (self.remove_dir)(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                // EBUSY: a process joined between the read and the rmdir.
                Err(_) => continue,
            }
            self.executions().released.remove(&execution_id);
        }
    }

    /// Move every process except `self_pid` from the supervisor cgroup into
    /// the jobs cgroup's shared leaf. Returns how many moved, or the first migration error
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
            match move_pid(&self.shared, pid) {
                Ok(true) => moved += 1,
                Ok(false) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
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

fn create_dir_idempotent(path: &Path) -> io::Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn parse_mem_total_bytes(meminfo: &str) -> Option<u64> {
    meminfo.lines().find_map(|line| {
        let kib = line.strip_prefix("MemTotal:")?.trim().strip_suffix("kB")?;
        kib.trim().parse::<u64>().ok()?.checked_mul(1024)
    })
}

/// Live isolation state for `/health`, plus per-execution placement. The
/// sweeper flips the status to `Failed` while migrations fail and back once
/// they succeed, so health never claims an isolation that is not being
/// enforced.
#[derive(Debug, Clone)]
pub struct JobCgroupHandle {
    status: Arc<RwLock<JobCgroupStatus>>,
    cgroup: Option<Arc<JobCgroup>>,
}

impl JobCgroupHandle {
    fn new(status: JobCgroupStatus) -> Self {
        Self {
            status: Arc::new(RwLock::new(status)),
            cgroup: None,
        }
    }

    /// No isolation: every execution method is a no-op.
    pub fn disabled() -> Self {
        Self::new(JobCgroupStatus::Disabled)
    }

    pub fn status(&self) -> JobCgroupStatus {
        self.status
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn set(&self, status: JobCgroupStatus) {
        *self
            .status
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = status;
    }

    /// Bound `execution_id`'s process tree, led by `leader_pid`, by the
    /// per-execution limits. A failure leaves the processes in the shared
    /// leaf, still inside the aggregate budget, and is logged.
    pub fn register_execution(&self, execution_id: Uuid, leader_pid: u32) {
        let Some(cgroup) = &self.cgroup else {
            return;
        };
        if let Err(error) = cgroup.register_execution(execution_id, leader_pid) {
            tracing::warn!(%execution_id, %error, "execution runs without its own cgroup limits");
        }
    }

    pub fn release_execution(&self, execution_id: Uuid) {
        if let Some(cgroup) = &self.cgroup {
            cgroup.release_execution(execution_id);
        }
    }

    /// Kill every process in the execution's cgroup. A no-op without one.
    pub fn kill_execution(&self, execution_id: Uuid) {
        if let Some(cgroup) = &self.cgroup
            && let Err(error) = cgroup.kill_execution(execution_id)
        {
            tracing::warn!(%execution_id, %error, "could not kill the execution cgroup");
        }
    }
}

/// Establish isolation, register the shared leaf for pre-exec placement, and
/// start the sweeper. Never fails the worker: a worker without isolation still
/// serves, and says so on `/health`.
pub fn start(config: Option<&JobCgroupConfig>, shutdown: CancellationToken) -> JobCgroupHandle {
    let Some(config) = config else {
        return JobCgroupHandle::disabled();
    };
    let (cgroup, isolated) = match establish_from_host(config) {
        Ok(established) => established,
        Err(error) => return JobCgroupHandle::new(failed(error)),
    };
    // Children spawned through the shared helpers join `jobs/shared` before
    // exec, so their memory is never charged to the supervisor; the sweeper
    // covers every other spawn path and moves executions into their own
    // cgroups.
    #[cfg(unix)]
    if !utils::command_ext::set_spawn_cgroup(&cgroup.shared.join("cgroup.procs")) {
        tracing::warn!("spawn cgroup already registered; relying on the sweeper alone");
    }
    let cgroup = Arc::new(cgroup);
    let mut handle = JobCgroupHandle::new(isolated.clone());
    handle.cgroup = Some(cgroup.clone());
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

fn isolated_status(
    cgroup: &JobCgroup,
    config: &JobCgroupConfig,
    mem_total: u64,
) -> JobCgroupStatus {
    JobCgroupStatus::Isolated {
        cgroup: cgroup.jobs.display().to_string(),
        memory_high: config.memory_high.resolve(mem_total),
        memory_max: config.memory_max.resolve(mem_total),
        execution_memory_high: cgroup.execution_memory_high.clone(),
        execution_memory_max: cgroup.execution_memory_max.clone(),
        weights: cgroup.weights.clone(),
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
    let status = isolated_status(&cgroup, config, mem_total);
    Ok((cgroup, status))
}

/// A plain thread, not a tokio task: it must keep moving children even when
/// the runtime is saturated, which is exactly when a runaway child appears.
fn spawn_sweeper(
    cgroup: Arc<JobCgroup>,
    isolated: JobCgroupStatus,
    handle: JobCgroupHandle,
    shutdown: CancellationToken,
) -> io::Result<()> {
    let self_pid = std::process::id();
    thread::Builder::new()
        .name("job-cgroup-sweeper".into())
        .spawn(move || {
            let mut failing = false;
            let mut placement_failing = false;
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
                // Per-execution placement is a refinement inside the
                // aggregate budget, so its failures are logged, not reported
                // as lost isolation.
                match cgroup.place_executions(&HostProcesses) {
                    Ok(_) => placement_failing = false,
                    Err(error) if !placement_failing => {
                        placement_failing = true;
                        tracing::warn!(%error, "could not move processes into their execution cgroup");
                    }
                    Err(_) => {}
                }
                cgroup.remove_released();
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
            execution_memory_high: MemoryLimit::PercentOfRam(40),
            execution_memory_max: MemoryLimit::PercentOfRam(50),
            cpu_weight: None,
            io_weight: None,
        }
    }

    struct FakeProcesses(HashMap<u32, (u32, u32)>);

    impl ProcessTable for FakeProcesses {
        fn parent_and_group(&self, pid: u32) -> Option<(u32, u32)> {
            self.0.get(&pid).copied()
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
        let shared_procs = cgroup.shared.join("cgroup.procs");
        assert_eq!(cgroup.sweep(200).unwrap(), 2);
        assert_eq!(fs::read_to_string(&shared_procs).unwrap(), "300");
        assert!(
            !cgroup.jobs.join("cgroup.procs").exists(),
            "jobs holds no processes"
        );
    }

    #[test]
    fn jobs_becomes_a_parent_with_a_shared_leaf_and_drains_old_members() {
        let (_root, supervisor) = fake_unit("memory");
        let jobs = supervisor.parent().unwrap().join("jobs");
        fs::create_dir(&jobs).unwrap();
        // Left behind by a worker running the single-leaf layout.
        fs::write(jobs.join("cgroup.procs"), "77\n").unwrap();
        fs::write(jobs.join("cgroup.subtree_control"), "").unwrap();

        let cgroup = JobCgroup::establish(&supervisor, &config(), 20 << 30).unwrap();
        assert_eq!(cgroup.shared, jobs.join("shared"));
        assert_eq!(
            fs::read_to_string(jobs.join("shared/cgroup.procs")).unwrap(),
            "77"
        );
        assert_eq!(
            fs::read_to_string(jobs.join("cgroup.subtree_control")).unwrap(),
            "+memory"
        );
        assert_eq!(cgroup.weights.cpu, "unset");
        assert_eq!(cgroup.weights.io, "unset");
    }

    #[test]
    fn weights_apply_where_the_controller_exists_and_report_otherwise() {
        let (_root, supervisor) = fake_unit("cpu memory");
        let mut config = config();
        config.cpu_weight = Some(50);
        config.io_weight = Some(50);
        let cgroup = JobCgroup::establish(&supervisor, &config, 1 << 30).unwrap();
        assert_eq!(cgroup.weights.cpu, "applied 50");
        assert!(
            cgroup.weights.io.starts_with("unavailable"),
            "{:?}",
            cgroup.weights
        );
        assert_eq!(
            fs::read_to_string(cgroup.jobs.join("cpu.weight")).unwrap(),
            "50"
        );
        assert!(!cgroup.jobs.join("io.weight").exists());

        assert_eq!(JobCgroupConfig::parse_weight("50"), Some(50));
        for invalid in ["0", "10001", "-1", "x", ""] {
            assert_eq!(JobCgroupConfig::parse_weight(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn registered_execution_gets_its_own_limited_cgroup() {
        let (_root, supervisor) = fake_unit("memory");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 20 << 30).unwrap();
        let execution_id = Uuid::new_v4();
        cgroup.register_execution(execution_id, 4242).unwrap();

        let path = cgroup.execution_path(execution_id);
        assert_eq!(path, cgroup.jobs.join(format!("exec-{execution_id}")));
        assert_eq!(
            fs::read_to_string(path.join("memory.high")).unwrap(),
            ((20u64 << 30) * 40 / 100).to_string()
        );
        assert_eq!(
            fs::read_to_string(path.join("memory.max")).unwrap(),
            ((20u64 << 30) * 50 / 100).to_string()
        );
        assert_eq!(
            fs::read_to_string(path.join("cgroup.procs")).unwrap(),
            "4242"
        );

        cgroup.kill_execution(execution_id).unwrap();
        assert_eq!(fs::read_to_string(path.join("cgroup.kill")).unwrap(), "1");
        // Killing an execution that never had a cgroup is not an error.
        cgroup.kill_execution(Uuid::new_v4()).unwrap();
    }

    #[test]
    fn placement_follows_process_group_and_ancestry_but_not_strangers() {
        let (_root, supervisor) = fake_unit("memory");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        cgroup.register_execution(first, 100).unwrap();
        cgroup.register_execution(second, 500).unwrap();
        // 101: in 100's group. 102 → 103: a setsid'd grandchild of 100.
        // 600: unrelated, reparented to init. 601: child of 500.
        let table = FakeProcesses(HashMap::from([
            (101, (100, 100)),
            (103, (100, 103)),
            (102, (103, 103)),
            (600, (1, 600)),
            (601, (500, 601)),
        ]));
        fs::write(cgroup.shared.join("cgroup.procs"), "101\n102\n600\n").unwrap();
        assert_eq!(cgroup.place_executions(&table).unwrap(), 2);
        // The fake file keeps the last write.
        assert_eq!(
            fs::read_to_string(cgroup.execution_path(first).join("cgroup.procs")).unwrap(),
            "102"
        );

        fs::write(cgroup.shared.join("cgroup.procs"), "601\n").unwrap();
        assert_eq!(cgroup.place_executions(&table).unwrap(), 1);
        assert_eq!(
            fs::read_to_string(cgroup.execution_path(second).join("cgroup.procs")).unwrap(),
            "601"
        );

        // A descendant whose ancestor already sits in an execution cgroup.
        let table = FakeProcesses(HashMap::from([(700, (601, 700))]));
        fs::write(cgroup.shared.join("cgroup.procs"), "700\n").unwrap();
        assert_eq!(cgroup.place_executions(&table).unwrap(), 1);
    }

    #[test]
    fn released_execution_cgroup_is_removed_once_empty() {
        let (_root, supervisor) = fake_unit("memory");
        let mut cgroup = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap();
        cgroup.remove_dir = |path| fs::remove_dir_all(path);
        let execution_id = Uuid::new_v4();
        cgroup.register_execution(execution_id, 100).unwrap();
        let path = cgroup.execution_path(execution_id);

        cgroup.release_execution(execution_id);
        cgroup.remove_released();
        assert!(path.exists(), "a cgroup that still holds a process stays");
        // Released executions are no longer placed.
        fs::write(cgroup.shared.join("cgroup.procs"), "100\n").unwrap();
        assert_eq!(
            cgroup
                .place_executions(&FakeProcesses(HashMap::new()))
                .unwrap(),
            0
        );

        fs::write(path.join("cgroup.procs"), "").unwrap();
        cgroup.remove_released();
        assert!(!path.exists());
        assert!(cgroup.executions().released.is_empty());
    }

    #[test]
    fn parses_parent_and_group_after_the_command_name() {
        assert_eq!(
            parse_parent_and_group("4242 (go (vet) x) S 4100 4000 4000 0 -1"),
            Some((4100, 4000))
        );
        assert_eq!(parse_parent_and_group("garbage"), None);
    }

    #[test]
    fn sweep_reports_migration_failures_instead_of_claiming_isolation() {
        let (_root, supervisor) = fake_unit("memory");
        let cgroup = JobCgroup::establish(&supervisor, &config(), 1 << 30).unwrap();
        fs::write(supervisor.join("cgroup.procs"), "100\n200\n").unwrap();
        // A directory where `cgroup.procs` should be makes every write fail.
        fs::create_dir(cgroup.shared.join("cgroup.procs")).unwrap();

        let error = cgroup.sweep(200).unwrap_err();
        assert!(error.to_string().contains("moving pid 100"), "{error}");

        let handle = JobCgroupHandle::new(JobCgroupStatus::Disabled);
        handle.set(failed(error));
        assert!(matches!(handle.status(), JobCgroupStatus::Failed { .. }));
    }
}
