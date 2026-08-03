# Clarifications: Legible Worker Executor Capabilities

`/speckit.clarify` resolved the four open questions in `spec.md` against the
project constitution (principles XI, XVIII, XXI, XXII), the existing cluster
knowledge-base pages, and the live homelab configuration.

## Decisions

### 1. Fail-closed at startup is correct for both invalid and empty lists (FR-1, FR-2)

Resolved: **fail closed in both cases.**

Reasoning:

- No live impact. Both workers are configured `executorProfiles = [ "CLAUDE_CODE" ]`
  (`hosts/think/think{3,4}.nix:122`), which is valid and canonical. Nothing in
  the current deployment starts failing.
- A worker with an unknown or empty list has *never* been schedulable for new
  work — `eligibility()` rejects it with `MissingExecutor` on every request. It
  is not a working node being taken away.
- The considered counter-argument: a worker also serves **already-placed**
  workspaces (sticky affinity, reconciliation, cancellation, log streaming), so
  refusing to start could strand them. Accepted as a real but bounded risk —
  such placements can only predate the misconfiguration, and the coordinator's
  existing reconciliation already treats an absent worker as *indeterminate*
  rather than complete (`docs/knowledge-base/workspace-directory-reclamation.md`).
  Making the node loudly absent is strictly better than leaving it present,
  reporting healthy in the admin UI, and quietly accepting nothing.
- The deploy path surfaces it: `wiki/self-hosted-deployment.md` requires a
  failed restart to be treated as a failed health check, with `OnFailure=`
  paging. A refusing worker pages; a silently-idle worker does not.

Constraint on the implementation: the error must name the offending value **and**
the valid executor names. A fail-closed error that does not say what to write
instead trades one silent failure for one loud but unactionable one. This
applies to the **empty** case too — `/speckit.analyze` (m14) caught that routing
it to the existing `Missing` variant would emit
`"VK_WORKER_EXECUTOR_PROFILES is required"`, which names neither. The empty case
gets its own message carrying the valid set.

### 2. An unsupported current selection is marked, never auto-switched (FR-7)

Resolved: **leave the selection as the user set it; surface the unsupported
state inline; do not block submit.**

Reasoning:

- Auto-switching would write through `useExecutorConfig`'s `onPersist` and
  silently change the user's remembered default agent. Constitution X reserves
  persistent writes for explicit submit actions.
- Blocking submit client-side would make the gate an enforcement point, which
  constitution XXII explicitly forbids — it must stay an affordance, because it
  reads a possibly-stale worker list and degrades open when it cannot parse.
- Therefore: unsupported options render disabled-with-reason in the list, and if
  the *currently selected* agent is unsupported, the same reason renders inline
  next to the picker. Send stays enabled and the server's FR-6 error is the
  backstop. The user is told before committing work, and is still the one who
  decides.

### 3. The available-profiles list reports advertisements verbatim (FR-6)

Resolved: **report the advertised strings exactly as workers published them**,
deduplicated and sorted; do not reduce `CODEX:PLAN` to `CODEX`.

Reasoning: constitution XXII forbids a consumer synthesising or widening a
capability its owner did not advertise, and principle XI requires diagnostics to
preserve backend-provided evidence rather than reinterpret it. Collapsing
`CODEX:PLAN` to `CODEX` would tell an operator the cluster runs Codex generally
when it runs exactly one variant. The advertised string is also the literal
value the operator must edit in Nix, which makes it the more actionable form.

### 4. No deprecation path is needed for non-canonical stored rows (FR-4)

Resolved: **silent tolerance; no warning log, no migration.** The decision
stands, but the reasoning below was **corrected during `/speckit.analyze`** — the
original justification was factually wrong and would have understated how load-
bearing FR-4 is.

Original (wrong) reasoning: that a stale non-canonical row is replaced within
seconds because every worker re-registers on restart, making FR-4 a transient
upgrade concern.

Corrected, verified against the code:

- `WorkerHeartbeat` (`crates/cluster-protocol/src/lib.rs:80-85`) carries
  `authority`, `resources`, `mount` and `jobs` — **no capabilities**. The
  registry's heartbeat path re-writes `capabilities: current.capabilities.0`
  (`crates/services/src/services/cluster/registry.rs:118`), i.e. it preserves
  whatever is already stored.
- Capabilities are therefore written **only** by `register`
  (`crates/worker/src/server.rs:139-143`).
- A coordinator-only upgrade — exactly what the homelab deploy does when think2
  restarts and think3/think4 keep running — leaves stale rows in place for the
  workers' **entire uptime**, potentially days.

So FR-4's tolerance is permanent, not transitional. That strengthens rather than
weakens the case for it: without it, a coordinator-first upgrade makes every
worker unschedulable until someone restarts it, which is precisely the outage
this feature exists to prevent.

Silent tolerance is still right. A warning would have to live in
`advertises_executor_profile`, which runs per worker per scheduling call, so the
log volume would scale with traffic rather than with the problem — and the
condition is not operator error, merely an older build.

**Constraint on T061**: the knowledge-base page must not repeat the "seconds"
claim. The heartbeat-does-not-carry-capabilities fact is the reusable insight
here.

## Consequences for the plan

- Step 2 must source the valid-name list for the error message from the
  canonical enumeration rather than hard-coding it, so it cannot drift from
  `BaseCodingAgent`.
- Step 4's `ExecutorUnsupported` variant carries `Vec<String>` of advertised
  strings, not parsed executors.
- Step 5 needs an inline unsupported notice for the current selection in
  addition to the per-option disabled state, and must not touch
  `setExecutorOverrides`.

## Remaining Questions

None blocking implementation.

Outside this feature and still open with the operator: how Codex authenticates
on the coordinator (ChatGPT device login writing `~/.codex/auth.json` versus
`OPENAI_API_KEY`), which determines the Nix plumbing needed to actually enable
Codex on think3/think4. Tracked in `spec.md` "Out of Scope".
