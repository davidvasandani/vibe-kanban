# Worker execution hardening: per-execution cgroups, pressure admission, startup-stall reaper

Why a cluster worker bounds each execution, refuses work under pressure, and
reaps agents that never start. Also the non-obvious traps found while
building it.

## The incident (think3, 2026-10-15)

think3 has 6 cores, 15 GiB of RAM and **no swap**. An agent's own `go build`
grew to ~50% of RAM inside the one aggregate `jobs` cgroup. Without swap,
reclaim can only evict file pages. The node thrashed (load 22, 78% iowait).
The next follow-up's `claude-code` then blocked in D-state during MCP init
and wrote nothing. The worker job stayed `Running` with no output for hours.
Stop could not kill it, because SIGKILL is only delivered once the I/O
completes. A queued `restart_session` waited for a finalization that never
happened.

`pgid = null` on the coordinator row was **not** the signal.
`execution_processes.pgid` is coordinator-local and is always null for
worker-owned rows. The worker's real evidence is "this job never produced a
byte".

## Shape

- **cgroups.** `<unit>/jobs` is now a *parent*. The pre-exec hook and the
  sweeper target `jobs/shared`. Each running execution gets
  `jobs/exec-<id>` with `VK_WORKER_EXECUTION_MEMORY_HIGH/MAX` (homelab
  sets 40%/50%). The 200 ms sweeper moves processes out of `shared` into
  their execution: by pgid == leader, or through the ppid chain (≤32
  hops) to the leader or to a pid already in that execution's cgroup. The
  ppid chain catches `setsid`'d descendants. Released cgroups are removed
  once their `cgroup.procs` is empty.
- **Weights.** `cpu.weight` / `io.weight` on `jobs` are best effort, and the
  outcome is reported on `/health.job_cgroup.weights`. The cluster disks
  run the `none` scheduler, so **`io.weight` is advisory** until BFQ or
  iocost is enabled.
- **Admission.** PSI `full avg60` for memory or IO ≥
  `VK_WORKER_ADMISSION_MAX_PRESSURE` (default 40) → 503 naming the
  resource. Replays of accepted work are answered *before* the check. The
  heartbeat carries `admission_refusing`. The scheduler adds +1000 to the
  score instead of excluding the worker, because placement is sticky and
  exclusion would fail workspace creation whenever every node is busy.
- **Reaper.** Coding-agent actions only. The timer is armed when the job
  enters `Starting`. After `VK_WORKER_STARTUP_STALL_SECONDS` (default 600)
  with `output_seen == false`, the reaper takes the cancellation guard,
  re-checks, SIGKILLs the group and `cgroup.kill`s the execution, then calls
  `finish_failed` with a `worker_error`. Reporting `Failed` is truthful
  because a process with SIGKILL pending never runs user code again. Drain
  accounting still counts the live child until it exits. `Failed` (not
  `Indeterminate`) also lets a queued MCP restart run in
  `finalize_remote_execution`.

## Gotchas

- **No-internal-process rule.** Once `jobs/cgroup.subtree_control` has
  `+memory`, writing a pid into `jobs/cgroup.procs` fails with EBUSY.
  Moving the pre-exec target to `jobs/shared` must land in the same change,
  and `establish` drains `jobs` → `shared` before enabling the controller.
- **A pending spawn has nothing to reap.** Codex round 1: publishing
  `Failed` while `spawn()` was still pending dropped drain accounting, and a
  late child could still start a turn. Round 2: *dropping* the spawn
  future is also unsafe. Amp and Cursor spawn a process group and then
  await a stdin write, so a drop leaks the group (`kill_on_drop` kills only
  the direct child). The rule: while `job.child` is `None`, emit one
  diagnostic, stay `Starting`/active, and reap after the hand-over. A spawn
  that returns into an already-terminal job is killed under the
  cancellation guard and never revives the job.
- **Cancellation stragglers.** A shell's background job ignores SIGINT. If
  the shell exits on SIGINT before the first poll, cancellation is
  "confirmed" with the `sleep` still alive. It shows up as a flaky
  `cancellation_kills_child_and_grandchild…` under parallel load.
  `confirmed()` now `killpg(SIGKILL)`s the remaining group, using the
  leader pid saved at spawn because `child.id()` is `None` after reaping,
  and also kills the execution cgroup.
- **Quiesced jobs are not stalls.** A session transfer SIGSTOPs the job.
  The reaper re-arms while `quiesced_by` is set.
- **Coordinator Indeterminate event.** The tracker's `Indeterminate` arm
  returned without `finalize_remote_execution`, unlike every sibling path.
  That left `ACTIVE_MCP_SESSION_RESTARTS` set, so `restart_session` kept
  answering `AlreadyInProgress`. It now finalizes.
- **An ordinary queued follow-up is dropped after any failed turn.** This
  is existing policy and was not changed. Recovery after a reap goes through
  `restart_session` or a fresh message.
- **Shared `CARGO_TARGET_DIR` across worktrees** can leave a stale
  `cluster-protocol` rlib in place: "no field named …" even though the
  source has it. Touch the file, or use separate target dirs.

## Verify on a node

`curl -s <node>:8086/health | jq '.job_cgroup, .pressure, .active_execution_count'`,
and `ls /sys/fs/cgroup/system.slice/vibe-kanban-worker.service/jobs/` should
list `shared` plus one `exec-<id>` per running execution.

## Diagnosing a pressure refusal (2026-10-19)

A follow-up on think4 failed at 20:10 UTC with IO `full avg60 41.6% >= 40%`.
The same session completed another execution on think4 at 20:19 UTC, before
any intervention. This was a transient admission refusal; the historical I/O
producer was not established. Current low pressure does not disprove the
prior refusal or prove which process caused it.

- Find the failed execution's `execution_worker_jobs.worker_node_id`, join it
  to `worker_nodes.hostname`, and correlate the coordinator journal timestamp.
  A workspace's existing placement remains sticky even when another worker is
  healthy. The scheduler's pressure penalty affects new placement.
- Check that worker's `/health.pressure`: resource averages, threshold and
  `admission_refusing`. Wait until pressure drops below the configured threshold
  and retry. `avg60` is a smoothed average, not a promise of recovery in 60 seconds.
- A dispatch makes at most one immediate retry for transient failures. That
  retry is unlikely to resolve sustained pressure. Do not increase the threshold
  or migrate a live session merely to hide the refusal.
- Dispatch errors now name the assigned hostname and UUID; persisted failure
  evidence includes `worker_hostname` and `worker_node_id`. Pressure errors
  include wait-and-retry guidance. These diagnostics do not change admission,
  replay, retry count or placement behavior.

## Contributed by

- `vk/f268-harden-cluster-w`
- `vk/bdd8-debug-vk-load`
