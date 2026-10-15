# Implementation plan: harden cluster workers

See `SPEC.md`. Steps in dependency order.

## A. Per-execution job cgroups (`crates/worker/src/job_cgroup.rs`)

1. **Config.** Extend `JobCgroupConfig` with `execution_memory_high`,
   `execution_memory_max` (`MemoryLimit`), `cpu_weight`, `io_weight`
   (`Option<u16>`, 1–10000). Parse them in `lib.rs::job_cgroup_from_lookup`
   from `VK_WORKER_EXECUTION_MEMORY_HIGH/MAX` and `VK_WORKER_JOB_CPU_WEIGHT`
   / `VK_WORKER_JOB_IO_WEIGHT`.
2. **Layout.** `establish` keeps creating `<unit>/jobs` with the aggregate
   limits. It then creates the default leaf `jobs/shared` and enables
   `+memory` (plus `+cpu`/`+io` when they are in `jobs/cgroup.controllers`)
   in `jobs/cgroup.subtree_control`. cgroup v2 forbids processes in `jobs`
   once its children have controllers, so pre-exec placement and the sweeper
   now target `jobs/shared`. Before enabling, any process still in `jobs`
   (a worker upgraded in place) is moved to `shared`.
3. **Weights.** Best effort: enable `+cpu`/`+io` on the unit's
   `subtree_control` when available, then write `cpu.weight` / `io.weight`
   on `jobs`. A failure is reported in the health status (`weights`) and is
   not fatal.
4. **Execution registry.** `JobCgroupHandle` gains an
   `Arc<Mutex<HashMap<Uuid, u32>>>` that maps each execution to its leader
   pid, plus `register_execution(id, pid)` / `release_execution(id)`.
   Register creates `jobs/exec-<id>` with the per-execution limits and moves
   the leader at once. Release marks the cgroup for removal.
5. **Sweep.** Each 200 ms pass: supervisor procs → `shared` (as today). Then
   every pid in `shared` whose pgid equals a registered leader's, or whose
   ppid chain (≤ 32 hops, via `/proc/<pid>/stat`) reaches a registered
   leader or a pid already in an `exec-*` cgroup, moves to that execution's
   cgroup. Released cgroups are `rmdir`ed once empty (`EBUSY` → retry next
   pass). The `/proc` reads go through a small trait/closure so tests can
   inject a fake process table.
6. **Kill.** `kill_execution(id)` writes `1` to `exec-<id>/cgroup.kill`.
   The reaper uses it to kill descendants that left the process group.

## B. Pressure admission (`crates/worker`)

7. `pressure.rs`: read `full avg60` from `/proc/pressure/{memory,io}`
   (reuse `node_metrics::parse::parse_pressure`). Add an
   `AdmissionPressure { threshold, reader }` whose `check()` returns
   `Ok(reading)` or `Err(PressureRefusal { resource, full_avg60 })`.
   Threshold from `VK_WORKER_ADMISSION_MAX_PRESSURE` (default 40, 0 = off).
8. `ExecutionSupervisor::dispatch`: after the idempotent-replay check and
   the drain check, call the pressure check →
   `ExecutionError::UnderPressure { resource, full_avg60, threshold }`,
   mapped to 503 in `worker_api.rs` with a clear message.
9. `ResourceSnapshot` (cluster-protocol) gains
   `#[serde(default)] memory_pressure_full_avg60: Option<f64>`,
   `io_pressure_full_avg60: Option<f64>`, `admission_refusing: bool`.
   `server.rs::resource_snapshot` fills them. `/health` gains a `pressure`
   object.
10. Scheduler `score`: add a large penalty (1000) when `admission_refusing`
    is true, so a refusing worker is chosen only when nothing else is
    eligible.

## C. Startup-stall reaper (`crates/worker/src/execution.rs`)

11. `WorkerJob` gains `output_seen: AtomicBool`, set by `stream_output`
    on the first non-empty read, and `started_at: Instant`.
12. The supervisor gets `startup_stall: Option<Duration>`, from
    `VK_WORKER_STARTUP_STALL_SECONDS` (default 600, 0 = off), plus a
    setter for tests. `run_job` spawns a watchdog task for coding-agent
    actions only (initial / follow-up / review). It sleeps for the window;
    if the job is still non-terminal and `output_seen` is false, it reaps:
    take the cancellation guard, re-check, signal SIGKILL to the group if
    a child exists, call `kill_execution` on the cgroup, then
    `finish_failed(job, None, "agent produced no output within {N}s of
    starting; reaped by the worker (node may be under memory/IO pressure)")`.
13. Late spawn: after `spawn` returns in `run_job`, if
    `job.state().await.is_terminal()`, kill the child group, wait briefly,
    and return without touching state.
14. Register and release the execution cgroup around the child's lifetime
    in `run_job`, through a handle stored on the supervisor
    (`Option<JobCgroupHandle>`, wired in `lib.rs`).

## D. Coordinator gap

15. `crates/local-deployment/src/container.rs`: in the tracker's
    `ExecutionEventPayload::Indeterminate` arm, call
    `container.finalize_remote_execution(execution_id).await` before
    `finish_msg_store`, as the sibling Indeterminate paths do.

## E. Deployment (homelab `modules/vibe-kanban-rebuild.nix`)

16. Add `worker.jobCgroup.executionMemoryHigh` (default `"40%"`),
    `executionMemoryMax` (default `"50%"`), `cpuWeight` (50) and
    `ioWeight` (50), and export them as the env vars above. The unit's
    `Delegate=memory` becomes `Delegate = "cpu io memory"` so the
    controllers are guaranteed in the unit's subtree.

## F. Tests

- job_cgroup: layout on a fake cgroupfs (shared leaf, subtree_control,
  exec cgroup limits, weights written, removal when empty), and a sweep
  with an injected process table (pgid match, ppid chain, unrelated stays).
- Config parsing for the new env vars.
- pressure parsing / threshold / disabled.
- dispatch refused under pressure, replay still accepted.
- Reaper: a silent `sleep` coding-agent-like job is reaped. The test hook
  lets a `Command` action opt in, because a real coding agent cannot run
  in tests. A job that prints output is not reaped. Late-spawn kill.
- Scheduler penalty ordering.

## G. Verify

`cargo test -p worker -p cluster-protocol -p services`, `cargo clippy`, `pnpm
run format`, `pnpm run check`, and for homelab
`nix-instantiate --parse` / its existing checks.
