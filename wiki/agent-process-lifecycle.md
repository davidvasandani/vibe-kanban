# Agent Process Lifecycle: Turn End, the Exit Monitor's Two Kill Points, and Keep-Warm

How a coding-agent turn ends at the process level, and the non-obvious traps
for anyone changing that lifecycle (e.g. keeping app-servers warm across
turns, VAS-107 fix #2).

## The identity chain

One turn = one `ExecutionProcess` row = (today) one OS process lifetime. Every
per-execution facility in `LocalContainerService`
(`crates/local-deployment/src/container.rs`) is a `HashMap<Uuid, …>` keyed by
the execution id: `child_store`, `msg_stores`, `db_stream_handles`,
`exit_monitor_handles`, `raw_log_tailers`, `cancellation_tokens`,
`adopted_pgids`. Making a process outlive its turn means re-attaching it to
the *next* turn's execution id — the streaming/log-normalization pipeline is
per-turn and must be rebuilt, not shared.

## Persistent app-server vs one-shot is implicit

There is no trait method or enum flag. The split is encoded solely as
`SpawnedChild.exit_signal: Some(..)` (Codex, OpenCode, and the ACP-backed
Gemini, Qwen, Copilot, and Grok executors — signals turn end over the wire) vs
`None` (Claude, Amp, Cursor, and Droid — exit naturally).
`ExecutionProcessRunReason::is_persistent()` is a *different* axis (true only
for `DevServer`/`BackgroundHelper`): it gates raw-log-to-file streaming and
boot re-adoption by pgid. `CodingAgent` is deliberately **non**-persistent
(asserted in tests) — don't flip that to keep an agent alive; it changes
streaming behavior. Use the narrow `SpawnedChild.keep_warm` capability
instead.

## Process liveness is not always turn liveness

The same `SpawnedChild.exit_signal` split governs final-output reconciliation.
For a natural-exit executor (`exit_signal: None`), a still-live owned child is
positive evidence that the turn remains active, so quiet final assistant output
must not trigger cleanup. For a signal-driven executor (`exit_signal: Some`),
the app-server child may legitimately outlive the protocol turn; child liveness
therefore proves only process liveness, not turn liveness.

If final assistant output becomes quiet and the signal-driven terminal event is
lost, the bounded reconciliation timer must be allowed to expire even while the
child lives. The exit monitor then reaps the exact owned group and records
`indeterminate`—never `completed`—so the authoritative process stream clears
the UI's stale Stop state. Applying the natural-exit liveness rule to both
lifecycle shapes re-arms the timer forever for Codex-style app servers.

Cluster execution has the same split at a second boundary. The coordinator's
worker-job lease proves that the worker process still owns the dispatch, but for
a signal-driven executor it does not prove that the protocol turn is active.
After quiet final output, a current lease may defer reconciliation only for a
natural-exit executor. Otherwise a lost terminal event plus regular lease
renewals re-arms the coordinator timer forever even after the worker-side child
rule is correct.

The worker must also retain `SpawnedChild.exit_signal`. Its generic execution
loop originally destructured `SpawnedChild` down to only `child` and
`mcp_refresh`, silently dropping the terminal channel, then polled only
`child.try_wait()`. That makes every signal-driven executor permanently running
on a cluster worker because its app-server is expected to stay alive. Worker
execution must select/poll both authorities: an executor signal classifies the
turn and reaps the process, while natural-exit commands and executors continue
to classify from OS exit status.

## The exit monitor kills twice

`spawn_exit_monitor` has **two** kill points; guarding only the obvious one
still reaps the process:

1. **Exit-signal branch**: on the executor's turn-completion oneshot it calls
   `kill_process_group` (SIGINT→SIGTERM→SIGKILL to the whole group via
   `killpg`, `crates/utils/src/process.rs`).
2. **Monitor tail**, after finalization (DB completion, commit, next-action,
   log flush): it `start_kill()`s the child "to SIGKILL orphaned children
   (e.g. MCP servers)" and then removes it from `child_store` — and removal
   drops the handle, whose `kill_on_drop(true)` kills again.

The keep-warm substrate (task 1a64) threads one `kept_warm` decision
(`should_keep_warm(keep_warm, is_success, was_stopped)` — clean success only)
past **both** points; failure, explicit stop, and non-warm executors keep
today's reap.

## The OS exit watcher is a poll loop, not a wait

`spawn_os_exit_watcher` polls `try_wait()` every 250ms and reports through a
oneshot. Dropping the receiver does **not** stop the task — it must check
`tx.is_closed()` each iteration or it spins forever against a deliberately
long-lived child (found by Codex review of the keep-warm change). General
rule: any watcher keyed to "process will die soon" breaks when a process is
allowed to live.

## Teardown only stops `Running` executions

Workspace/attempt teardown (`try_stop`) calls `stop_execution` only for
executions whose DB status is `Running`. A warm child's turn row is
`Completed`, so once `keep_warm` is enabled the warm registry must own
reaping at attempt/workspace end explicitly — otherwise the app-server
outlives its deleted worktree. Phase 2 (task 826e) closed this: a
`ContainerService::reap_warm_processes_for_session` hook (default no-op,
overridden by `LocalContainerService`) is called from `try_stop` for **every**
session regardless of status, plus `stop_execution` reaps by session key, and
`kill_all_running_processes` (shutdown) drains the whole registry — because a
warm `CodingAgent` is `Completed` (missed by `find_running()`) **and** is not in
the persistent-process boot re-adoption path, so nothing else would reclaim it.

## Phase 2 warm registry: the reuse mechanism and its concurrency traps

The warm registry (`warm_app_servers: HashMap<session_id, WarmAppServer>` on
`LocalContainerService`, task 826e) is the single owner of a kept-warm process.
Non-obvious gotchas found building it (several via adversarial Codex review):

- **`base_url` is discovered asynchronously**, *after* `spawn` returns (OpenCode
  prints its listening URL from the spawned task). So the reuse handle can't ride
  on `SpawnedChild` synchronously — it's surfaced over a
  `oneshot` (`SpawnedChild.warm_reuse`). At warm-turn-end the value is already
  sent, so the exit monitor reads it with `try_recv` (not `.await`) to avoid any
  suspension window.
- **Park before finalization, not after.** The exit monitor must move the child
  into the registry *before* it runs `try_start_next_action`/queued-follow-up
  dispatch, or an immediately chained follow-up misses the warm entry and
  cold-starts.
- **Insert-before-remove (no invisibility gap).** `park_warm_child` inserts into
  the registry *before* removing from `child_store`, so a concurrent teardown
  always finds the child in at least one owner — never a gap where it is in
  neither and leaks.
- **Owned child via `Arc::try_unwrap`.** The registry holds
  `Arc<RwLock<AsyncGroupChild>>`; reuse needs an owned `AsyncGroupChild`
  (`SpawnedChild.child`). Once removed from the registry the Arc is uniquely
  owned, so `try_unwrap` succeeds — but **only** because the idle sweep is
  written to *not* clone child handles. If a sweep held a transient clone,
  `try_unwrap` would fail and (naively) kill a healthy server. On the
  should-not-happen shared case, re-park instead of killing.
- **Generation-conditional reap.** The idle sweep collects stale candidates under
  the registry read lock, drops it, then reaps — so it must reap *only if the
  entry's `last_active` still matches* (`reap_warm_entry_if_unchanged`), else it
  can kill a server that was reaped-and-re-registered in the window.
- **Never hold the registry lock across a child-kill/`try_wait` await** (and vice
  versa) — reap removes under the map lock, then kills after dropping it.
- **Cold-start fallback must be clean.** A warm follow-up validates the server
  synchronously (`check_warm_server_ready`) before reuse and, on any pre-spawn
  error, kills the server's *process group* explicitly (not `kill_on_drop`, which
  only reaps the direct child and is treated as unreliable here) so the container
  can cold-start with no orphan. An alive-but-hung server thus cold-starts instead
  of failing the turn.
- **Re-tracking a warm child** installs a fresh stdout pipe, but its stderr was
  consumed by the first turn's forwarder — so `track_child_msgs_in_store` must
  tolerate an absent stream (treat as empty) rather than `expect` it.
- **Enablement is gated** behind env `VK_KEEP_WARM_AGENTS` (default off): the
  executor declares `keep_warm: true`, the container ANDs it with the gate, so
  default behavior is byte-for-byte unchanged (the live reuse path is
  unobservable in CI — Constitution IV). Idle window default 30 min, swept every
  5 min.
- **Phase 3 decisions (task 826e):** Codex warm reuse is deferred — its
  `turn/completed` is coupled to the reader-loop teardown (breaking the loop →
  `send_exit_signal` → kill), so warming it needs an out-of-band per-turn
  completion signal. ACP is deliberately left one-shot — resume already replays a
  `.jsonl` transcript into a fresh process, so warm reuse buys the least.

## Queued follow-ups must survive early finalization

Queued messages are normally claimed in the exit monitor's general finalization
block. A coding-agent action may carry a cleanup `next_action`, so
`should_finalize` is false until that cleanup finishes. When the coding turn
makes no repository changes, the monitor deliberately skips cleanup and manually
finalizes instead. Any pending handoff normally performed by the bypassed block
must happen before that manual finalization.

Concretely, claim and start the queued follow-up before setting the
`already_finalized` guard. Otherwise the message remains in the in-memory
`QueuedMessageService` forever even though the task is complete. Share scratch
deletion and queued execution start logic with the normal consumer, and fall back
to finalization if the queue is empty/cancelled or execution start fails. The
general lesson is that an early-finalization shortcut must audit and preserve
all handoffs owned by the block it bypasses, not only completion notification.

## `ScheduleWakeup` is denied, not honored (VAS-283)

The Claude Code harness offers a `ScheduleWakeup` tool: the agent parks its turn
and expects a supervising loop to re-invoke it after a delay. VK has no such
loop — a turn is one process and it is reaped at turn end (both kill points
above), so any in-process wake-up timer dies with it. Left alone the tool
returns *success*, the agent ends its turn having done nothing, and the
execution is recorded `completed` / `exit_code 0` — a silent task abandonment.

`ScheduleWakeup` is purely a harness-side tool: VK never receives the request as
an actionable event, it only sees the `ToolUse` in the normalized log stream. So
the fix lives at the tool-permission boundary VK *does* control when it launches
the CLI (`crates/executors/src/executors/claude.rs`), as two independent layers:

1. **`--disallowedTools=ScheduleWakeup`** in every permission mode
   (`build_command_builder`) — the harness-native "tool unavailable" signal.
2. **A PreToolUse `deny` hook** matching `^ScheduleWakeup$` in every mode
   (`get_hooks` → `DENY_SCHEDULE_WAKEUP_CALLBACK_ID`, handled in
   `claude/client.rs::on_hook_callback`). The deny is checked **before** the
   `auto_approve` short-circuit so it also fires in bypass/auto mode — the mode
   the incident occurred in — and the catch-all approve/ask matchers explicitly
   exclude `ScheduleWakeup` so the deny is unambiguous. The reason string
   (`SCHEDULE_WAKEUP_DENY_REASON`) tells the agent to do the work inline or leave
   a follow-up, so it keeps working instead of parking.

Residual dependency (unverifiable from this repo): both layers assume the CLI
subjects the harness-injected `ScheduleWakeup` tool to `--disallowedTools` /
PreToolUse hooks like a normal registry tool. It appears in the tool registry
like any other tool, so this holds in practice, but a harness change could
bypass both — in which case the CLI-independent fallback is to detect the
`ScheduleWakeup` `ToolUse` during log normalization and surface it.

This is the cheap "make the failure honest" guardrail (VAS-283 option A). The
real capability — persist `(session_id, fire_at, prompt)` and re-invoke via the
existing `--resume` + `QueuedMessageService` paths (option B) — is deferred to
be designed alongside VAS-132 ("Issue Background Tasks as Sub-Issues"), which
covers the same gap (agent-initiated work that outlives the current turn) from
the background-subagent angle.

**Generalized to the whole background-poller class** — see [[vk-pollers]].
`ScheduleWakeup` is one member of it; the others (Claude's
`Bash(run_in_background)` + `Monitor`/`TaskOutput`/`TaskStop`, Codex's
`unified_exec` `exec_command`/`write_stdin` session) are closed the same way, and
the replacement is a VK-owned poller riding this page's background-helper/pgid
substrate. Two findings from that work generalize back here: denying tool *names*
is insufficient when the vendor's real path runs through a **parameter**
(`run_in_background`) plus an undeniable tool (`Read` on the output file); and a
vendor identifier must be read from the artifact that actually executes — the
Claude npm package is a stub whose `sdk-tools.d.ts` lists schema titles, not wire
tool names. Option B remains deferred: a vk poller runs a command, not a turn.

## What already survives restarts (reuse, don't reinvent)

`ExecutionProcess.pgid` is persisted at spawn; boot-time adoption
(`adopted_pgids`, `kill_orphan_process_group`, `process_group_alive`) reclaims
or cleans up groups from a previous server instance — built for dev servers
(PR #55), extended to background helpers (VAS-111/PR #84). Warm agents and the
deferred Tier-3 restart-survival designs (exec-in-place upgrade vs
supervisor/runner split — trade-offs in
`homelab/specs/vk/1a64-coding-agent-pro/research.md`) all build on this pgid
substrate.

## Enablement order (why OpenCode first)

OpenCode's child is an HTTP server; its stdout matters only until the
listening URL is printed, and turns stream over HTTP — so re-attach needs only
the stored `base_url` + password. Codex (stdio JSON-RPC; turn end currently
*breaks the reader loop*) and ACP (connection dropped after the turn-end
`cancel`) need their transports kept alive across turns.

## Contributed by

- vk/1a64-coding-agent-pro
- vk/826e-coding-agent-war
- vk/9f36-vk-queued-messag
- vk/869c-vk-background-po
- vk/7655-turn-ends-aren-t
