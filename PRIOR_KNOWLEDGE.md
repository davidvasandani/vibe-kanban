# Prior Knowledge — recalled for `vk/826e-coding-agent-war`

Searched the project knowledge base (`wiki/` — 12 topic pages + INDEX) for pages
relevant to this task: keeping persistent coding-agent app-servers (OpenCode/
Codex/ACP) warm across turns, and owning their reaping at teardown.

## Result: one directly on-topic page (the Phase-1 page for this same line)

- **[agent-process-lifecycle.md]** — the single most relevant page, contributed by
  the immediately-preceding task `vk/1a64-coding-agent-pro` (Phase 1). It is the
  authoritative recall for this task and was used directly. Key facts carried in:
  - **The identity chain** — one turn = one `ExecutionProcess` = (today) one OS
    process lifetime; per-execution facilities are `HashMap<Uuid, …>` keyed by
    exec id. Making a process outlive its turn means re-attaching it to the next
    turn's execution — the streaming/normalization pipeline is per-turn and must
    be rebuilt. → Drove the design of the reuse path re-wrapping the same child in
    a fresh `SpawnedChild` that flows through the identical downstream pipeline.
  - **Persistent app-server vs one-shot is implicit** — encoded only as
    `SpawnedChild.exit_signal: Some(..)`; `is_persistent()` is a *different* axis
    (dev servers / background helpers) that must NOT be flipped for coding agents.
    Use the narrow `keep_warm` capability. → We added `warm_reuse` alongside
    `keep_warm`, not a new run-reason.
  - **The exit monitor kills twice** (exit-signal `killpg` + tail
    `start_kill`/`child_store.remove`); both already thread `kept_warm`. → We
    changed only the tail: on `kept_warm`, MOVE the child into the registry
    instead of leaving it in `child_store`.
  - **The OS exit watcher is a poll loop** that must check `tx.is_closed()` or it
    spins against a long-lived child. → Confirmed the parked-child move doesn't
    reintroduce a spin (the watcher breaks once the monitor's rx drops).
  - **Teardown only stops `Running` executions** — the warm-process trap: a warm
    child's row is `Completed`, so the registry must own reaping at attempt/
    workspace end. → This task's central deliverable: the `try_stop` reap hook.
  - **The pgid re-adoption substrate** already reclaims/cleans process groups
    across a restart. → Reused for shutdown/restart rather than a new scheme; also
    the shared foundation for the deferred Tier-3 designs recorded on that page.

## Tangentially related

- **[self-hosted-deployment.md]** — the versioned-release deploy contract; relevant
  only to Tier-3 (the deploy *trigger* for exec-in-place vs. supervisor split lives
  in that contract). Noted in `tier3-restart-survival.md`, not otherwise used.

## Constraints carried into design (from the constitution)

- **Stay mergeable with upstream** → all hot-file edits (`container.rs`, `mod.rs`)
  are additive (one `SpawnedChild` field, one registry map + helpers, one
  `try_stop` hook); no rewrites, no generated-file edits, no migration.
- **Turn is a protocol event** (VK-fork principle, sharpened this task to name the
  `try_stop`-only-`Running` leak) → the registry-as-single-reaper is the direct
  implementation.
- **Never break a running service (IV)** → live reuse gated off by default because
  it is unobservable E2E here.

## Knowledge to record in stage 12
This task extends **agent-process-lifecycle.md** with the Phase-2 mechanism
(the warm registry, the reaping owner that closes the leak, the gate, the async
reuse-handle surfacing, and the Codex/ACP Phase-3 decisions) and adds this task
id to its "Contributed by" list.
