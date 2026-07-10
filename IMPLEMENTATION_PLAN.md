# Implementation Plan: Coding-Agent Process Survival (task vk/1a64-coding-agent-pro)

Step-by-step build order for Phase 1 (warm-app-server substrate, default-off).
The authoritative dependency-ordered task list is
`homelab/specs/vk/1a64-coding-agent-pro/tasks.md` (T001/T003/T007/T008/T009 in
this phase); this is the executable narrative. Rationale in `SPEC.md`;
prior-art recall in `PRIOR_KNOWLEDGE.md` at the workspace root.

## Step 1 — Carrier (T001)

In `crates/executors/src/executors/mod.rs`, add `keep_warm: bool` to
`SpawnedChild` with a doc comment stating the contract (persistent app-server;
clean turn end keeps the process). Default `false` in
`From<AsyncGroupChild>`. Set it explicitly at all six construction sites —
`stdout_dup.rs`, `claude.rs`, `codex.rs`, `opencode.rs`, `acp/harness.rs`
(×2) — `false` everywhere in Phase 1, with a "Phase 2/3 enables this" comment
on the three persistent executors.

## Step 2 — Exit-monitor decision (T003)

In `crates/local-deployment/src/container.rs`:

1. Capture `spawned.keep_warm` in the spawn path and pass it to
   `spawn_exit_monitor(&exec_id, exit_signal, keep_warm)`.
2. Add the pure helper `should_keep_warm(keep_warm, is_success, was_stopped)`
   near the other free helpers.
3. In the monitor, hoist `let mut kept_warm = false;` above the
   `tokio::select!`. In the exit-signal arm compute
   `kept_warm = should_keep_warm(keep_warm, is_success,
   ExecutionProcess::was_stopped(...))`; skip `kill_process_group` when it
   holds (log it), otherwise keep today's kill.
4. **Also guard the monitor tail** — the post-finalization
   `start_kill()` + `child_store.remove()` block must be skipped when
   `kept_warm`, or the warm child is silently reaped anyway (second kill
   point; `kill_on_drop` on removal). The warm child stays in `child_store`
   so the existing stop/teardown owner can still reach it.
5. Leave the entire finalize path (update_completion, commit, next-action,
   session summary, raw-log flush, msg-store cleanup) untouched.

## Step 3 — Tests (T007)

`#[cfg(test)] mod warm_tests` in `container.rs`: the four decision-matrix
cases (warm+success+!stopped → kept; warm+failure, warm+success+stopped, and
all !warm cells → reaped). Run `cargo test -p local-deployment warm`.

## Step 4 — Docs + no-churn check (T008)

Confirm nothing here is `#[derive(TS)]`-exported or persisted (no
`generate-types`, no migration). Comments explain *why* (two kill points,
protocol-event principle), not *what*.

## Step 5 — Gates (T009)

`cargo fmt --all` (no diff expected), `cargo clippy -p executors
-p local-deployment --all-targets`, `cargo test -p executors
-p local-deployment`, `pnpm i && pnpm run check && pnpm run lint`.

## Deferred (recorded in tasks.md Phases 2-4)

Warm registry keyed by attempt + reap/idle/liveness (T100), OpenCode reuse via
stored `base_url` + `keep_warm = true` (T101-T102), Codex/ACP transport
keep-alive (T201-T202), Tier-3 restart survival (T301).
