# Prior knowledge: harden cluster workers (job cgroups + stuck-execution reaper)

Task `vk/f268-harden-cluster-w`. Distilled from `wiki/` (read-only search for
cgroup, admission, stuck, D-state, Indeterminate, heartbeat, worker).

## Relevant pages

- **coordinator-nfs-load.md**: load average and io PSI under-count NFS
  waits. Count D-state *threads* directly; `node-metrics` reports them as
  `cpu.uninterruptible_tasks`. So PSI is a good signal for local-disk
  thrash, but not the only possible one. That fits the incident: local
  page-cache thrash with no swap, 78% iowait.
- **coordinator-restart-handoff.md**: the coordinator follows every worker
  job with one long-poll tracker per execution. Worker restart → recovered
  jobs become `Interrupted` (chat shows Resume). A replay gap or regression
  resolves from terminal evidence or is classified `Indeterminate`
  (constitution XVIII). Known gap: the heartbeat does not carry
  `admission_draining`, so a refusing worker still looks schedulable, and a
  dispatch to it fails instead of going elsewhere. Placement is sticky after
  workspace creation.
- **agent-process-lifecycle.md**: one turn = one `ExecutionProcess`. Process
  liveness is turn evidence only for natural-exit executors. Indeterminate
  is recorded rather than inventing Completed. `pgid` re-adoption is a
  **coordinator-local** concept, so worker-owned rows always have
  `pgid = null`. A null pgid is therefore not itself a wedge signal.
- **vk-pollers.md**: bound the *effect*, not a name. A silent foreground
  loop hung a turn for an hour, and the `BASH_*_TIMEOUT_MS` clamp bounds
  in-turn commands. A "no output yet" watchdog must not misfire on agents
  that are legitimately quiet *after* they started. Only "never produced
  anything" is safe to reap.
- **worker-journal-agent-stream-boundary.md**: worker `Structured` metadata
  (`worker_error`, `cancellation_phase`, `stream_error`) is rendered as a
  diagnostic, never as agent stdout. A reaper reason belongs in
  `worker_error`.

## Prior decisions to respect

- Job cgroup isolation (`crates/worker/src/job_cgroup.rs`): the supervisor
  cgroup is never limited. Children join `jobs` before exec through
  `utils::command_ext::place_in_spawn_cgroup`, and a 200 ms sweeper thread
  (a plain thread, not a tokio task) is the backstop. cgroup v2 does not
  migrate existing charges.
- Drain safety: `active_execution_count` deliberately counts terminal jobs
  whose child is still alive (quarantine case). Do not "decrement" it while
  a process lives.

## Not found

No page covers per-execution resource limits, PSI admission control, or a
startup-stall watchdog. This task adds that knowledge.
