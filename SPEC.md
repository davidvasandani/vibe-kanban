# Harden cluster workers: per-execution job cgroups, pressure admission, startup-stall reaper

Task: `vk/f268-harden-cluster-w`. Vibe Kanban service, plus the worker unit's
cgroup delegation in homelab `modules/vibe-kanban-rebuild.nix` (the VK
deployment). Node swap for think3–5 is host configuration, not the VK
service, and is out of scope (recorded as a follow-up).

## Problem

On 2026-10-15 a workspace on think3 (6 cores, 15 GiB, no swap) wedged:
queued follow-ups, no reply, a Stop button that did nothing.

- The agent's own `go vet`/`go build` grew to ~50% of RAM and ran 4+ hours.
- With no swap, memory pressure can only evict file pages. The node
  thrashed (load ~22, ~78% iowait, ~17 tasks in D-state).
- The next follow-up's `claude-code` blocked in D-state during MCP init and
  never wrote a byte. The worker job sat in `Running` with no output, and
  `active_execution_count` stayed 1.
- Stop / `restart_session` could not finish: SIGKILL to a D-state process is
  not delivered until its I/O completes.
- Recovery needed root on the node.

### What the code does today (verified)

- `crates/worker/src/job_cgroup.rs` creates **one** aggregate `<unit>/jobs`
  cgroup. On think3 it gets `memory.high` ~65% and `memory.max` ~80% of RAM.
  Every agent, build and MCP server on the node shares that one budget. A
  single runaway build can fill it and push every co-resident agent into
  reclaim. There is no per-execution bound, and no CPU or IO weighting
  (`Delegate=memory` only).
- `crates/worker/src/execution.rs::run_job` has no watchdog. A job whose
  process never produces output stays `Running` forever. A job whose
  `spawn()` never returns stays `Starting` with no child handle, so
  cancellation has nothing to signal.
- If `spawn()` returns after the job was already made terminal, `run_job`
  overwrites the state with `Running`.
- Admission (`dispatch`) refuses only while draining. A node that is
  thrashing still accepts new agents and wedges them.
- `pgid` on `execution_processes` is a coordinator-local field. It is always
  null for worker-owned executions, so "pgid = null" is not itself the
  signal. The signal is "the worker job never produced output".

## Goals

1. **Per-execution memory bound.** Every coding-agent execution's process
   tree runs in its own `<unit>/jobs/exec-<execution_id>` cgroup, with
   its own `memory.high` / `memory.max` (configurable, e.g. 45% / 55% of
   RAM). A runaway build is throttled and then OOM-killed inside its own
   execution, before the aggregate `jobs` budget is used up. The aggregate
   limits stay as the outer bound.
2. **CPU/IO weighting (best effort).** If the unit delegates `cpu` / `io`,
   the worker gives `jobs` a lower `cpu.weight` / `io.weight` than its own
   supervisor cgroup, so the control plane keeps running under contention.
   The weights are reported on `/health`. `io.weight` only takes effect
   with an IO controller that honours it (BFQ or iocost). The cluster
   disks use the `none` scheduler, so the health status says when the
   setting is advisory.
3. **Pressure admission control.** While node PSI (`/proc/pressure/memory`
   or `/proc/pressure/io`, `full avg60`) is at or above a threshold
   (default 40%), the worker refuses new dispatches with 503 and a
   distinct error. Idempotent replays of an already-accepted execution
   still succeed. Pressure readings and the refusal state go on `/health`
   and into the heartbeat `ResourceSnapshot`, so the scheduler stops
   placing work on a thrashing node.
4. **Startup-stall reaper.** A coding-agent execution that has produced no
   stdout/stderr since it started is reaped after
   `VK_WORKER_STARTUP_STALL_SECONDS` (default 600; 0 disables). If `spawn()`
   itself has not returned, nothing is published as terminal. An executor
   can already own a process group that dropping its future would leak
   (Amp/Cursor await a stdin write after spawning). Instead the reaper
   records one visible `worker_error`, keeps the job owned and active, and
   reaps it once the child is handed over. The reaper:
   - kills the process group, and `cgroup.kill` the execution cgroup if
     there is one, so D-state processes die as soon as their I/O completes;
   - records a structured `worker_error` naming the cause
     ("produced no output within N s of starting; reaped");
   - transitions the job to `Failed` with terminal evidence. The
     coordinator then finalizes the execution and queued follow-ups
     dispatch (see the coordinator-side notes in IMPLEMENTATION_PLAN.md).

   `active_execution_count` keeps counting a reaped job while its process
   is still alive. That count is drain-safety evidence (XVIII): a release
   drain must not pretend a live process is gone. It drops to 0 as soon
   as the kill lands. The session no longer waits on it, because the job
   is terminal.
5. **Late spawn is killed, not resurrected.** If `spawn()` returns after
   the job became terminal (reaped or cancelled), `run_job` kills the new
   child at once and does not change the state.

### Coordinator behaviour this relies on (verified)

- A worker `Failed` event finalizes the row as `failed` and runs
  `finalize_remote_execution`. A queued **MCP restart** (`restart_session`)
  then executes (`container.rs` `finalize_remote_execution`). An ordinary
  queued follow-up is dropped after any failed turn, which is existing
  policy and is not changed here. So once the reaper fails the stalled job,
  a pending `restart_session` runs, which is the recovery path.
- Gap fixed here: on a worker-reported `Indeterminate` *event* (cancellation
  whose SIGKILL was not confirmed), the tracker returned without
  `finalize_remote_execution`. Restart tracking (`ACTIVE_MCP_SESSION_RESTARTS`)
  was never cleared, so later `restart_session` calls returned
  `AlreadyInProgress` forever. Every sibling Indeterminate path already
  finalizes, so this path now does too.
- Dispatch is sticky to the workspace's worker. A 503 is retried once and
  then the execution fails with the worker's message. The pressure refusal
  message therefore says what happened ("worker is under memory/IO
  pressure …"). The scheduler is only used at placement, and a refusing
  worker is scored last there rather than excluded, so placement never
  fails just because every node is busy.

## Non-goals

- Swap/zram on think3–5. That is node configuration outside the VK service.
- Reaping executions that produced output and then went quiet. A long
  silent tool call is legitimate, and pollers / the bash-timeout clamp
  already bound those.
- Coordinator-side DB watchdog for worker-owned rows. The coordinator
  already follows worker evidence. Inventing a terminal state without
  worker evidence would break XVIII.
- Killing orphaned background processes at the end of a turn. Keep-warm
  and background helpers rely on processes outliving a turn.

## Configuration (worker env)

| Variable | Default | Meaning |
|---|---|---|
| `VK_WORKER_EXECUTION_MEMORY_HIGH` | unset (= `max`) | per-execution `memory.high` (`max`, bytes+K/M/G/T, `N%`) |
| `VK_WORKER_EXECUTION_MEMORY_MAX` | unset (= `max`) | per-execution `memory.max` |
| `VK_WORKER_JOB_CPU_WEIGHT` | unset (no write) | `cpu.weight` of `jobs` (1–10000) |
| `VK_WORKER_JOB_IO_WEIGHT` | unset (no write) | `io.weight` of `jobs` (1–10000) |
| `VK_WORKER_ADMISSION_MAX_PRESSURE` | `40` | refuse dispatch at/above this PSI `full avg60` %; `0` disables |
| `VK_WORKER_STARTUP_STALL_SECONDS` | `600` | reap a silent coding-agent execution after N s; `0` disables |

The per-execution cgroups exist only when job-cgroup isolation is on
(`VK_WORKER_JOB_CGROUP`). Without it, the reaper and admission control
still work.

## Acceptance

- A1: With isolation on, a coding-agent execution's process group lands in
  `jobs/exec-<id>`, whose `memory.high`/`memory.max` match configuration;
  the cgroup is removed when empty after the job ends (unit tests on a fake
  cgroupfs).
- A2: Processes that are not part of an execution land in `jobs/shared`.
  `jobs` itself holds no processes, which cgroup v2's no-internal-process
  rule requires once `jobs` has children with controllers enabled.
- A3: The worker refuses a new dispatch with 503 `pressure` while the PSI
  reading is over the threshold, and accepts a replay of an existing
  execution (unit test with an injected reading).
- A4: A coding-agent job with no output for the stall window becomes
  `Failed` with a `worker_error` naming the stall. Its process group is
  killed. A job that produced output is never reaped. A stalled spawn gets
  one diagnostic and stays non-terminal and active (unit tests, short
  window).
- A5: A spawn that completes after the job is terminal is killed and the
  state is unchanged (unit test).
- A6: Health/heartbeat expose pressure, admission refusal and
  per-execution cgroup configuration; the scheduler skips workers whose
  heartbeat reports pressure refusal.
- A7: `pnpm run check`, `cargo test` for the touched crates, `pnpm run lint`
  and `pnpm run format` are clean.
