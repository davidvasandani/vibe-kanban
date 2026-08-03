# Prior Knowledge: Worker Executor Capability Advertisement

The knowledge base is populated and was searched before planning.

## Which knowledge base

There are **two** parallel knowledge bases with **zero filename overlap** — they
are not duplicates and neither is a superset:

- `docs/knowledge-base/` (20 pages) — **canonical for this pipeline.** ~15 task
  folders under `specs/` carry explicit tasks of the form
  *"Update docs/knowledge-base/ and its index"* (e.g.
  `specs/vk/957e-clustered-vibe-k/tasks.md:173`). It owns the clustering page.
  Attribution is a `` Tags: `id` `` line directly under the H1; the index is a
  markdown table (`| Page | Summary | Contributing tasks |`) with link text
  **excluding** `.md`.
- `wiki/` (18 pages) — a live second KB from a non-speckit task stream, product
  and UI leaning. Referenced by one task file only. It uniquely owns
  `self-hosted-deployment.md` and `project-context-map.md`. Attribution is a
  trailing `## Contributed by` section; the index is a bullet list with link
  text **including** `.md`.

Neither is linked from `AGENTS.md`/`CLAUDE.md`. New pages for this task belong
in `docs/knowledge-base/`, cross-referencing `wiki/self-hosted-deployment.md`.

**There is no existing page covering worker capability advertisement.** That is
the gap this task should fill.

## Most relevant pages

- `docs/knowledge-base/clustered-workspace-execution.md`
- `wiki/self-hosted-deployment.md`
- `docs/knowledge-base/cli-tool-oauth-login.md`
- `wiki/managed-cli-tool-catalog.md`
- `docs/knowledge-base/grok-executor-integration.md`
- `docs/knowledge-base/executor-model-catalog-maintenance.md`
- `docs/knowledge-base/workspace-environment-inheritance.md`

## Distilled guidance

1. **Prove a capability; do not assert it.** *"Treat a shared mount as a
   capability, not a directory."* Before becoming schedulable a worker verifies
   the NFS export, a coordinator-issued probe, writability, and storage-side
   UID/GID ownership — *"an existing path does not prove that NFS is mounted."*
   Executor profiles are the one capability that stayed a hand-typed env string,
   and that is precisely the anti-pattern this page warns against.

2. **Capabilities are worker-authored and coordinator-consumed.** The
   coordinator is authoritative for placement, SQLite, worktrees and approvals;
   a worker owns only its own processes. The coordinator must therefore **not
   synthesise or widen** an advertised set. Canonicalising for comparison is
   acceptable; inferring capability is not.

3. **Never retry a dispatch on a different worker**, and never infer affinity
   from the currently selected UI host. A `NoEligibleWorkers` rejection cannot
   be papered over by failing to another node — the fix belongs at advertisement
   and registration time.

4. **Stale state must not reach user-facing reads.** *"Expiring stale `online`
   rows only inside scheduler selection leaves an admin UI claiming a dead
   worker is healthy."* The same hazard applies to capabilities: a UI reading
   profiles from a stale registration row will mislead. This is why R4's
   client-side gate is explicitly an affordance and R3 stays the enforcement
   point.

5. **A load-bearing list needs a completeness test.** `CliToolId::ALL` drives
   listing, locks and catalog tests, and *"forgetting to add a new variant there
   makes the tool effectively invisible even if a catalog entry exists."*
   `VK_WORKER_EXECUTOR_PROFILES` is the same shape of bug, one config layer out.
   Model genuinely-unsupported as a first-class status rather than an error —
   *"unsupported ≠ error"*.

6. **Backend-only compatibility checks are insufficient.** *"Backend-only
   compatibility checks leave the UI able to construct saves that the backend
   rejects."* Directly motivates R4. The grok page's companion lesson: a
   deployment-level list that silently misses an executor is a real, shipped
   failure mode (the `ExecutorApprovalBridge` omission that made a no-op service
   approve every tool request).

7. **Auth failures must be actionable, never silent.** Try auth methods in
   preference order and continue past a failure — *"a stale cached login must
   not prevent `XAI_API_KEY` authentication"* — and complete the execution with
   an actionable message. Bears directly on the deferred Codex-enablement work.

8. **Agent CLI auth is host-side and per-node.** *"Vibe Kanban should not become
   an OAuth client or credential store."* Offer in-app login only when
   credentials survive the login child process **and** a separate non-secret
   command can verify them. Two traps for the deferred work: *"a path alone can
   target the UI machine by mistake"* — logging in from the coordinator UI
   authenticates think2, not think3/think4 — and *"a zero exit becomes success
   only after the independent probe confirms authentication."*

9. **Credentials cannot ride in via org env vars.** The resolver filters
   reserved keys including `VK_*`, `PATH`, `HOME` and **executor auth wiring**,
   so the Codex auth path must be explicit. Also note *"multiple process
   boundaries per workspace"*: `ContainerService` and `PtyService` are separate,
   and fixing only the executor path leaves terminals with a different
   environment.

10. **Nix secrets take absolute paths, reject `/nix/store/`, and load through
    systemd credentials** — *"a Nix path literal can copy a secret into the
    world-readable store."* This is why the Codex auth-shape question (device
    login file vs. `OPENAI_API_KEY`) actually changes the design and cannot be
    guessed.

11. **The deploy gate is real.** Deploy machinery lives in the homelab repo's
    `modules/vibe-kanban-rebuild.nix`; releases publish to `/srv/vk-releases`
    with an atomic `current` flip. *"Passing local tests does not replace"* a
    two-node deployment exercise. Edge triggers stall silently — *"binaries from
    commit N−1 are 'present' while commit N is silently undeployed"* — so if a
    worker still advertises the old list after this change, suspect deployment
    before logic. Health is `/api/health` for the server (bare `/health` is
    answered 200 by the SPA fallback).

12. **Rejected alternatives not to re-propose** (from `self-hosted-deployment.md`):
    a Nix-native flake package with comin, containers, and blue/green. Also
    rejected elsewhere: YAML or a `.nix` attrset for machine-readable topology
    (JSON + `jq`, because *"a config file doesn't justify a new parser
    dependency"*).

## Consequences for this task

- R1 (validate at startup) and R2 (canonicalising match) are the minimum
  expression of guidance 1 and 5 that does not violate guidance 2.
- A full availability probe would better satisfy guidance 1 but is **deferred**:
  guidance 8 explains why a file-presence probe would report `NotFound` for
  Claude Code on these workers, which authenticate by env token — it would
  unschedule the only working agent. Any probe must be advisory.
- Guidance 11 means R1's fail-closed startup behaviour needs confirming on a
  real worker; it converts a running-but-useless worker into a refusing service.
