# Technical Spec: Coding-Agent Process Survival (warm app-servers)

> Task vk/1a64-coding-agent-pro. Full SpecKit artifacts live in
> `homelab/specs/vk/1a64-coding-agent-pro/` (`spec.md`, `plan.md`,
> `research.md`, `data-model.md`, `tasks.md`). This file is the repo-root
> technical summary.

## Problem

Persistent app-server executors (Codex, OpenCode, ACP) run a long-lived
process and signal turn completion **over the wire** instead of exiting. VK
ends every turn by killing the agent's whole process group and respawns a
fresh process next turn, discarding warm state and paying a cold start
(VAS-107 fix #2):

- `spawn_exit_monitor`'s exit-signal branch reaps the group unconditionally
  (`crates/local-deployment/src/container.rs`, via `kill_process_group` →
  `killpg` in `crates/utils/src/process.rs`).
- The monitor tail *also* `start_kill()`s the child and drops it from
  `child_store` — a second, less obvious kill point (plus `kill_on_drop`).
- The persistent/one-shot distinction exists only implicitly as
  `SpawnedChild.exit_signal: Some` vs `None`;
  `ExecutionProcessRunReason::CodingAgent` is deliberately non-persistent.

## Goal

Make a completed turn a **protocol event, not process death** (constitution
v0.5.1, Vibe Kanban fork section): finalize the turn's records while keeping
the app-server process warm for the next turn — while failure, explicit stop,
attempt end, and shutdown still reap it exactly as today, and one-shot CLI
executors (Claude, Gemini, Amp, Cursor, Qwen, Copilot, Droid) are completely
unaffected.

Full server-restart survival (exec-in-place upgrade / supervisor split) stays
**deferred Tier-3 scope** — recorded in the SpecKit research notes, not
built; `--resume` + persisted `agent_session_id` already recover the work
across restarts, so only the tail of one in-flight turn is at stake.

## What ships in this task (Phase 1 — substrate, default-off)

1. **Carrier**: `SpawnedChild.keep_warm: bool`
   (`crates/executors/src/executors/mod.rs`), default `false`, explicit at
   all six construction sites. One-shot executors never set it.
2. **Decision**: `spawn_exit_monitor` takes `keep_warm`; a pure helper
   `should_keep_warm(keep_warm, is_success, was_stopped)` gates BOTH kill
   points (the exit-signal branch's `kill_process_group` AND the monitor
   tail's `start_kill` + `child_store.remove`). On a clean warm turn the
   child stays alive and in `child_store`, so the existing stop/teardown
   owner can still reach it. Every per-turn side effect is preserved: the
   `ExecutionProcess` reaches a terminal status, the session id persists,
   raw logs flush, queued follow-ups start.
3. **Tests**: `warm_tests` in `container.rs` cover the full decision matrix
   (warm+success kept; warm+failure, warm+stopped, and every non-warm cell
   reaped).

No executor sets `keep_warm = true` yet, so **runtime behavior is
unchanged**. This is deliberate: enabling reuse requires the Phase 2
re-attach work below, which must be verified against a live agent.

## What lands next (recorded, out of scope here)

- **Phase 2**: `warm_app_servers` registry keyed by task attempt (child,
  pgid, executor reuse handle, `last_active`); reap on attempt end + idle
  timeout (default 30 min) + out-of-band-death detection; OpenCode first
  (its warm asset is an HTTP `base_url` — least surgery), with
  `spawn_follow_up` attaching instead of spawning. At most one warm process
  per attempt; turns strictly sequential.
- **Phase 3**: Codex (don't break the JSON-RPC reader loop on
  `turn/completed`) and ACP (keep the connection after the turn-end
  `cancel`).
- **Phase 4 (Tier 3)**: exec-in-place or supervisor split, built on the same
  pgid-tracked, re-adoptable warm-process substrate.

## Acceptance (Phase 1)

- Decision-matrix unit tests pass (`cargo test -p local-deployment warm`).
- fmt/clippy/check gates pass; no generated-file edits; no migration.
- Default-path behavior unchanged (no executor opts in yet).
- Diff stays minimal and additive in hot upstream files (fork mergeability).
