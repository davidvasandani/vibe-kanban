# Worker capability advertisement

Tags: `9f3d-cluster-worker-p`

How a cluster worker declares which coding agents it can run, why that
declaration is the one capability in the subsystem that was *asserted* rather
than *proved*, and the failure modes that follow. Companion to
[clustered-workspace-execution](clustered-workspace-execution.md), which covers
placement, dispatch and mount health; deployment context lives in
`wiki/self-hosted-deployment.md`.

## The asymmetry that causes the bug

One side of the comparison is generated, the other is typed by a human:

- The **request** is `ExecutorProfileId`'s `Display` output, always canonical
  and always qualified. The frontend composes it as
  `` `${executor}:${variant ?? 'DEFAULT'}` `` — so `CODEX:DEFAULT`, never
  `codex`.
- The **advertisement** is whatever an operator wrote into
  `VK_WORKER_EXECUTOR_PROFILES`, forwarded verbatim into
  `WorkerCapabilities.executor_profiles` as a plain `Vec<String>`.

Comparing those byte-for-byte makes `codex` and `CODEX` different capabilities.
Nothing rejects the mismatch: the worker registers, heartbeats, reports healthy,
and is simply never selected. The operator sees a green node that silently never
receives work, and the user sees `no eligible worker supports executor profile
"CODEX:DEFAULT"` after writing a prompt.

Canonicalise on both sides, at the boundary. Validate where the value is
authored — the worker resolves each entry through `BaseCodingAgent::from_str`
and fails startup on an unknown name, listing the valid ones.

## Capabilities are written only at registration

`WorkerHeartbeat` carries `authority`, `resources`, `mount` and `jobs` — **no
capabilities** — and the registry's heartbeat path re-writes
`capabilities: current.capabilities.0`, preserving what is already stored. Only
`register` updates them.

Consequences that are easy to get wrong:

- **Tolerance for non-canonical stored values is permanent, not transitional.**
  A coordinator-only restart (the normal deploy shape when one node redeploys
  and the workers keep running) leaves stale rows for the workers' entire
  uptime — days, not seconds. A "we'll canonicalise on the next heartbeat"
  assumption is false.
- Upgrade ordering: worker-first is safe. Coordinator-first is safe *only*
  because the consumer tolerates old values; without that, every worker becomes
  unschedulable until individually restarted.

## Fail closed at startup, but say what to write instead

An empty or misspelled list has no useful degraded mode — the worker can never
be selected — so refusing to start is better than running uselessly. Two
conditions make that safe rather than hostile:

1. The message names the offending value **and** the valid set. A fail-closed
   error that does not say what to write trades a silent failure for a loud
   unactionable one. Source the valid list from the enumeration itself so it
   cannot drift.
2. Roll out staged — one worker, confirm registration, then the next.
   Fail-closed startup plus a simultaneous multi-node deploy can take the whole
   cluster down at once if a latent misconfiguration exists on more than one.

`crates/worker` has no `strum` dependency and `executors` re-exports only
`strum_macros`, so `CodingAgent::VARIANTS` is **not** readable from the worker.
Expose the list as a helper from `executors::profile` instead of adding a
top-level dependency. (`VariantNames` is derived on the outer `CodingAgent`, not
on the `BaseCodingAgent` discriminants.)

## Authoring may normalise; consuming may not

`CODEX:` names a variant — the empty one. A worker canonicalising *its own*
config may collapse that to bare `CODEX`, because pinning "the variant whose
name is the empty string" is never what an operator meant. The **scheduler must
not**: widening someone else's advertisement grants capability its owner never
declared. Keep the parse faithful (variant is `Some` iff a `':'` was present)
and let only the authoring helper drop it.

Same rule for error messages: report advertised profiles **verbatim**. Reducing
`CODEX:PLAN` to `CODEX` tells an operator the cluster runs Codex generally when
it runs exactly one variant, and the advertised string is the literal value they
must edit.

## Distinguish "cannot" from "unavailable"

An empty candidate set has two causes with opposite remedies, and one message
for both sends people the wrong way:

- Healthy workers exist, none advertises the agent → name the requested profile
  and the profiles that *are* available. Remedy: switch agent, or advertise it.
- No healthy worker at all → report the count and a per-reason tally, and do
  **not** mention the executor. Remedy: go fix the nodes.

A worker rejected *only* for `MissingExecutor` is otherwise healthy, which is
what makes the executor the actionable fact. In a mixed population prefer the
executor explanation — switching agent would work right now.

The manually-pinned path needs the same treatment. Formatting `{reason:?}`
renders the bare Rust identifier `MissingExecutor`, naming nothing the user can
act on.

## A UI capability gate must degrade open

Mirroring the check in the browser stops a user writing a prompt into a dead
end, but it reads possibly-stale data over a field typed `unknown`
(`#[ts(type = "unknown")]` on `capabilities`). Rules that follow:

- Return "no opinion" (`null`), not an empty set, when nothing parses. An empty
  set reads as "this cluster runs nothing" and disables every agent — worse than
  the error it replaces.
- **Never withdraw a capability that works.** Two ways this was nearly shipped:
  forgetting the `CURSOR` → `CURSOR_AGENT` alias (both parse on the backend, so
  a `CURSOR` row really can run the agent the picker calls `CURSOR_AGENT`); and
  treating an omitted variant as "no variant", which made a worker advertising
  exactly `CODEX:DEFAULT` look unsupported even though the request *is*
  `CODEX:DEFAULT`. When the variant is unknown, ask only whether some variant of
  that executor is runnable.
- Do not auto-switch the user's selection — that persists a default they did not
  choose — and do not block submit. The gate is an affordance; the coordinator
  stays the enforcement point.
- Skip lease expiry client-side. It would degrade *closed* against a clock the
  browser does not own, and expired workers are marked offline anyway.

A disabled dropdown row sets `pointer-events-none`, so a `title` tooltip never
appears. Put the reason in visible content.

## Verification pattern

- **Pin the old predicate against the new one exhaustively.** Keep a copy of the
  pre-change matching function in the test module and assert every
  (advertised, requested) pair over a realistic alphabet agrees, except an
  explicit list of intended fixes. This is what proves a canonicalisation
  rewrite did not quietly change an unrelated case; reading the diff does not.
- **Fix unrealistic fixtures before adding the regression test.** The scheduler
  tests advertised `["codex", "claude"]` and requested bare lowercase names —
  neither producible by the running system. Both sides being equally unrealistic
  is exactly why the case-sensitivity bug passed tests for so long. Migrating
  them to canonical values *first* is what makes a subsequent
  legacy-tolerance test meaningful rather than a duplicate of the default path.
- Cover all four scheduling populations: healthy-but-unsupported, all-unhealthy,
  zero workers, and mixed.
- Cover every degenerate capability shape in the UI helper — absent, null,
  non-object, missing list, non-array list, array of non-strings — asserting the
  gate permits everything.
- A pure-function test cannot observe a disabled option or a rendered reason;
  add a DOM test for the picker, and check the assertion discriminates (one
  option disabled, another not).
- `SchedulingError` is not a ts-rs type, so `generate-types:check` should show
  **no** diff — treat any diff as an accidental export.
