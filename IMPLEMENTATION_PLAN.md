# Implementation Plan: Warm coding-agent process reuse (task vk/826e-coding-agent-war)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/826e-coding-agent-war/tasks.md`; this is the repo-root summary.

## Goal

Give Phase 1's dormant keep-warm substrate its first real user (OpenCode) and the
reaping owner the constitution requires before it can be enabled: a container-owned,
session-keyed warm registry that reuses a live OpenCode server across turns and
reaps it at teardown/stop/idle/death — closing the documented `try_stop` leak.
Backend-only; live reuse gated **off** by default (`VK_KEEP_WARM_AGENTS`).

## Steps (build order)

### Increment A — registry + reaping owner + gate (default-off, fully unit-tested)

1. **`WarmReuseHandle` + `SpawnedChild.warm_reuse`** — new struct (redacted
   `Debug`) + optional `oneshot` receiver field, default `None`. Add the field to
   all 6 `SpawnedChild` literals (`None` everywhere except OpenCode).
   `crates/executors/src/executors/mod.rs` (+ `stdout_dup.rs`, `claude.rs`,
   `codex.rs`, `acp/harness.rs`). (T001)

2. **OpenCode declares + surfaces** — `keep_warm: true`; send
   `{base_url, server_password, agent_session_id}` on the channel right after URL
   discovery. `crates/executors/src/executors/opencode.rs`. (T002)

3. **Registry + gate on the container** — `warm_app_servers: HashMap<session_id,
   WarmAppServer>` field + `WarmAppServer` type (`is_alive`) + `KEEP_WARM_ENV` /
   `WARM_IDLE_TIMEOUT` consts + `parse_keep_warm`/`keep_warm_env_enabled`. Init in
   `new`. `container.rs`. (T003)

4. **Reap/register/take/sweep** as module-level free functions (testable) with
   thin `&self` delegators; `warm_entry_is_idle` pure predicate. `container.rs`. (T004)

5. **Populate on warm turn end** — thread effective keep-warm
   (`spawned.keep_warm && warm_agents_enabled()`) + `warm_reuse` + `session_id`
   into `spawn_exit_monitor`; on `kept_warm`, move child `child_store` → registry.
   `container.rs`. (T005)

6. **Reap on explicit stop** — `stop_execution` reaps the session's warm entry. (T006)

7. **Reap on teardown (the leak fix)** — `ContainerService::reap_warm_processes_for_session`
   trait method (default no-op), called in `try_stop` for every session regardless
   of `Completed`; `LocalContainerService` overrides it. `services/container.rs`
   + `container.rs`. (T007)

8. **Idle sweep** — dedicated 5-min level-triggered task in `spawn_workspace_cleanup`
   calling `sweep_idle_warm_servers`. (T008)

9. *(T009 not wired — no graceful shutdown hook exists; `kill_on_drop` + boot pgid
   re-adoption cover shutdown. Documented.)*

### Increment B — OpenCode reuse hot path (behind the gate, compile-verified)

10. **`Opencode::warm_follow_up`** — install a fresh per-turn pipe on the warm
    child, stream `run_session` over HTTP to the stored `base_url`, return a
    `SpawnedChild` re-wrapping the same child. `opencode.rs`. (T010)

11. **Intercept `start_execution_inner`** — on gate-on + OpenCode follow-up + live
    warm entry, `take_live_warm_server` → `spawn_warm_follow_up`; the `SpawnedChild`
    flows through the unchanged downstream pipeline. Miss/dead → cold start.
    `container.rs` + `coding_agent_follow_up.rs` resolver. (T011)

### Tests + gates

12. **Unit tests** — `warm_tests`: gate parsing, idle predicate, register→take,
    one-per-session, reap kills+removes, idempotent reap, dead→miss, idle sweep
    (stale + dead), and `WarmReuseHandle` `Debug` redaction. (T012/T013)

13. **Gates** — `cargo test -p executors -p services -p local-deployment`,
    `cargo clippy`, `cargo fmt`. (T015)

## Non-goals / deliberately deferred

- Codex reuse (documented seam), ACP reuse (decision: one-shot), Tier-3
  implementation (design only) — see `research.md` / `tier3-restart-survival.md`.
- Enabling the gate on by default (unobservable E2E here — Constitution IV).
- Any DB migration / generated-type change (the new field is internal).

## Status

All Increment A + B steps complete and compiling; 16 warm tests + full suites for
the three crates pass (executors 84, services 25, local-deployment 58 + 1
pre-existing ignored); clippy + fmt clean. Live reuse gated off. Independent Codex
review ran 5 rounds — every confirmed finding fixed (stderr-panic on reuse, the
register/sweep concurrency races, `try_unwrap`-kills-live, warm-error cold-start
fallback, shutdown reap, park-before-finalization + insert-before-remove) — final
verdict **NO SIGNIFICANT FINDINGS**. Knowledge distilled into
`wiki/agent-process-lifecycle.md` (stage 12).
