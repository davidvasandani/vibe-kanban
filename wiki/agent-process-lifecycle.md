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
`SpawnedChild.exit_signal: Some(..)` (Codex, OpenCode, ACP — long-lived
process, signals turn end over the wire) vs `None` (Claude, Gemini, Amp,
Cursor, Qwen, Copilot, Droid — exit naturally).
`ExecutionProcessRunReason::is_persistent()` is a *different* axis (true only
for `DevServer`/`BackgroundHelper`): it gates raw-log-to-file streaming and
boot re-adoption by pgid. `CodingAgent` is deliberately **non**-persistent
(asserted in tests) — don't flip that to keep an agent alive; it changes
streaming behavior. Use the narrow `SpawnedChild.keep_warm` capability
instead.

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
outlives its deleted worktree. (Tracked as the Phase-2 must-cover in
`homelab/specs/vk/1a64-coding-agent-pro/tasks.md` T100.)

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
