# Technical Spec: Warm coding-agent process reuse (Phase 2/3) + Tier-3 design

> Task `826e-coding-agent-war`. Full SpecKit artifacts live in
> `homelab/specs/vk/826e-coding-agent-war/` (`spec.md`, `plan.md`, `research.md`,
> `data-model.md`, `tasks.md`, `tier3-restart-survival.md`). This file is the
> repo-root technical summary. Builds on Phase 1 (`specs/vk/1a64-coding-agent-pro/`,
> PR #92).

## Problem

A completed coding-agent turn is a **protocol event, not process death**. For
persistent app-servers (OpenCode/Codex/ACP) VK ends every turn by killing the
process group, then respawns next turn — throwing away warm state and paying a
cold start (VAS-107 fix #2). Phase 1 shipped the substrate to *decline* that kill
(`SpawnedChild.keep_warm` + `should_keep_warm`, default-off), but nothing set it
and — critically — nothing **owned** the resulting warm process: a warm turn's
`ExecutionProcess` row is `Completed`, and teardown (`try_stop`) only stops
`Running` rows, so enabling keep-warm without an owner would leak the app-server
past its deleted worktree (the `container.rs:962` NOTE / 1a64 T100).

## Solution

Backend-only, additive, gated **off by default** (env `VK_KEEP_WARM_AGENTS`).

### 1. Container-owned warm registry + reaping owner (the core, fully unit-tested)
`LocalContainerService` gains `warm_app_servers: HashMap<session_id,
WarmAppServer>`. The registry is the **single reaper** of a warm process's
lifetime, closing the leak. Reap triggers:
- **`try_stop` teardown** — a new `ContainerService::reap_warm_processes_for_session`
  hook (default no-op) is called for every session **regardless of `Completed`
  status**. This is the leak fix.
- **`stop_execution`** (explicit stop/cancel), **idle-timeout** (30 min, via a
  5-min level-triggered sweep), **out-of-band death** (liveness `try_wait` before
  reuse), and process-exit `kill_on_drop` + boot pgid re-adoption for shutdown.
The core ops are module-level free functions (`register_warm_entry`,
`reap_warm_entry`, `take_live_warm_entry`, `sweep_idle_warm_entries`,
`warm_entry_is_idle`, `parse_keep_warm`) so they are unit-tested against real
child processes without a DB-backed container.

### 2. Populate on clean warm turn end
The exit monitor already computes `kept_warm`. Effective keep-warm =
`spawned.keep_warm && warm_agents_enabled()` (container gates the executor's
declared capability). When warm, the monitor **moves** the child out of
`child_store` into `warm_app_servers[session_id]` with the reuse handle the
executor surfaced.

### 3. Surfacing the reuse handle (async problem)
OpenCode's `base_url` is discovered *asynchronously* after `spawn` returns, so it
can't ride on `SpawnedChild` synchronously. Added `SpawnedChild.warm_reuse:
Option<oneshot::Receiver<WarmReuseHandle>>` (default `None`); OpenCode sends
`{base_url, server_password, agent_session_id}` once discovered. `WarmReuseHandle`
redacts `server_password` from `Debug` (credentials-never-logged).

### 4. OpenCode reuse hot path (Increment B, behind the gate)
`Opencode::warm_follow_up` installs a fresh per-turn stdout pipe on the warm child
and streams the turn over HTTP to the stored `base_url` (no new `opencode serve`
boot). `start_execution_inner` intercepts an OpenCode follow-up with a live warm
entry and calls it via `CodingAgentFollowUpRequest::spawn_warm_follow_up`; the
resulting `SpawnedChild` flows through the **identical** downstream pipeline
(pgid record, msg tracking, exit monitor). A miss/dead entry reaps itself and
cold-starts.

### Phase 3 (Codex + ACP) — decisions, not code
- **Codex**: documented the seam (decouple `turn/completed` from the reader-loop
  teardown in `client.rs`/`jsonrpc.rs`) and deferred; `keep_warm` stays `false`.
- **ACP**: **left one-shot** (decision) — resume already replays the `.jsonl`
  transcript into a fresh process, so warm reuse buys the least; the redundant
  post-`cancel` kill is noted for a future cleanup, not removed.

### Tier 3 — deferred design only
`tier3-restart-survival.md` picks the supervisor/runner split as the eventual
target (if zero-interruption deploys become a goal), defines the pipe-FD/handoff
contract, and names the deploy trigger.

## Why gated off

This environment cannot run OpenCode/Codex/ACP end-to-end, so the reuse hot
path's acceptance criteria are unobservable here (Constitution IV — do not enable
a runtime path you cannot observe). Default behavior is byte-for-byte unchanged;
flipping `VK_KEEP_WARM_AGENTS` on is the E2E-verification step (1a64 T102) where
OpenCode can run. The registry, reaping, liveness, gate, and idle logic are proven
by unit tests that run in CI.

## Scope / non-goals

- No DB migration, no generated-type change (the new field is internal). The
  durable identity (`execution_processes.pgid`) already exists.
- No one-shot executor change (Claude/Gemini/Amp/Cursor/Qwen/Copilot/Droid).
- No Codex reuse, no ACP reuse, no Tier-3 implementation (all documented).

## Validation

- `cargo test -p executors -p services -p local-deployment` — all pass (executors
  84, services 23, local-deployment 58 + 1 pre-existing ignored), plus 13
  registry/gate/idle tests + 1 redaction test added.
- `cargo clippy` clean; `cargo fmt` clean.
- Full `pnpm run check`/`lint`/`cargo test --workspace` are the CI-side gate.

## Files

- `crates/executors/src/executors/mod.rs` — `WarmReuseHandle`, `SpawnedChild.warm_reuse`, redaction test.
- `crates/executors/src/executors/opencode.rs` — declare `keep_warm`, surface handle, `warm_follow_up`.
- `crates/executors/src/actions/coding_agent_follow_up.rs` — `spawn_warm_follow_up` resolver.
- `crates/executors/src/executors/{codex,acp/harness,claude,stdout_dup}.rs` — new field (`None`), Phase-3 decision comments.
- `crates/local-deployment/src/container.rs` — registry, gate, reap/register/take/sweep, populate-on-warm-end, interception, tests.
- `crates/services/src/services/container.rs` — `try_stop` reap hook (trait default no-op).
