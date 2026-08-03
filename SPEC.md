# Technical Spec: Legible Worker Executor Capabilities

## Problem

Creating a workspace with the Codex agent on the clustered self-hosted
deployment fails after submit with:

```
no eligible worker supports executor profile "CODEX:DEFAULT"
```

The scheduler is behaving correctly. The failure is a configuration and
legibility problem, not a scheduling bug.

### Observed chain

1. `crates/server/src/routes/workspaces/create.rs:362` — when clustering is
   enabled, *every* workspace creation must be placed on a worker. There is no
   coordinator-local fallback.
2. The requested profile is stringified via `ExecutorProfileId`'s `Display`
   impl (`crates/executors/src/profile.rs:111`) as `CODEX:DEFAULT`.
   `BaseCodingAgent` renders SCREAMING_SNAKE_CASE.
3. `WorkerScheduler::select` filters by `eligibility()`
   (`crates/services/src/services/cluster/scheduler.rs:110`), which requires the
   worker's `capabilities.executor_profiles` to contain `CODEX:DEFAULT` or the
   bare `CODEX`.
4. Workers advertise verbatim whatever the operator placed in
   `VK_WORKER_EXECUTOR_PROFILES` (`crates/worker/src/lib.rs:116`,
   `crates/worker/src/server.rs:141`). Nothing validates, canonicalises, or
   verifies the list.
5. In the governing homelab module both workers are configured
   `executorProfiles = [ "CLAUDE_CODE" ]` (`hosts/think/think3.nix:122`,
   `hosts/think/think4.nix:122`).

`CODEX:DEFAULT` therefore matches nothing, `NoEligibleWorkers` is returned as a
400, and the create dialog renders it via `createWorkspace.error`.

This is not Codex-specific. Enabling clustering silently reduced the deployment
to a single agent; Grok, Gemini, Copilot and the rest fail identically. The
coordinator (think2) has an installed, authenticated `codex` CLI, but
coordinator-local execution is unreachable once `clusterRole = "coordinator"`.

### Defects this spec addresses

- **D1 — Unvalidated advertisement.** An operator can advertise an executor name
  that does not exist. `lookup(WORKER_EXECUTOR_PROFILES_ENV).unwrap_or_default()`
  means an unset or empty variable produces a worker that registers, reports
  healthy, appears online in the admin UI, and is eligible for nothing. Silent.
- **D2 — Case-sensitive matching.** `advertises_executor_profile` compares
  `&str` values directly. `executorProfiles = [ "codex" ]` is accepted by Nix,
  registered by the worker, and never matches `CODEX:DEFAULT`. The scheduler's
  own unit tests use lowercase `"codex"` throughout — self-consistent, but not
  data the running system can produce.
- **D3 — Dead-end error.** The message names only the profile that failed. It
  does not say what the cluster *can* run, and it cannot distinguish "no worker
  has this executor" from "every worker is offline / unhealthy mount / expired
  lease". Those need opposite remedies.
- **D4 — Unreachable option offered.** The create-mode executor picker offers
  every configured agent regardless of cluster capability, so the failure only
  surfaces after the user has written a prompt.

## Guiding prior art

From `docs/knowledge-base/clustered-workspace-execution.md`:

- *"Treat a shared mount as a capability, not a directory."* Mount health is
  **proved** before a worker becomes schedulable. Executor profiles are the one
  capability that stayed an asserted env string.
- The coordinator is authoritative for placement, but capabilities are
  **worker-authored and coordinator-consumed**. The coordinator must not
  synthesise or widen a worker's advertised set.
- *"Never retry a dispatch on a different worker."* A `NoEligibleWorkers`
  rejection cannot be papered over by failing to another node; the fix belongs
  at advertisement and registration time.

From `wiki/managed-cli-tool-catalog.md`: `CliToolId::ALL` is load-bearing, and
forgetting an entry *"makes the tool effectively invisible even if a catalog
entry exists"* — answered there with a **completeness test**. Same idiom
applies.

From `docs/knowledge-base/grok-executor-integration.md`: *"Backend-only
compatibility checks leave the UI able to construct saves that the backend
rejects."* D4 is exactly that.

## Scope

In scope, all within this repository:

- Worker-side validation and canonicalisation of `VK_WORKER_EXECUTOR_PROFILES`.
- Case-insensitive, canonicalising profile matching in the scheduler.
- A scheduling error that distinguishes cause and names the cluster's supported
  profiles.
- Create-mode UI gating of executors no eligible worker can run.

Explicitly out of scope:

- **Provisioning Codex credentials on think3/think4**, and the homelab module
  changes that would accompany them (`codex` in the worker unit's `path`, a
  credential mechanism paralleling `opClaudeOauthTokenRef`, and flipping
  `executorProfiles`). Blocked on an unanswered question: whether Codex on
  think2 authenticates via a ChatGPT device login writing `~/.codex/auth.json`
  or via `OPENAI_API_KEY`. The two need different Nix plumbing —
  `clustered-workspace-execution.md` requires secret options to take absolute
  paths, reject `/nix/store/` paths, and load through systemd credentials.
- **Auto-de-advertising unavailable executors.** Tempting given the "prove,
  don't assert" principle, and `get_availability_info()` already exists per
  executor (`crates/executors/src/executors/codex.rs:264`). Rejected for now:
  Claude Code on the workers authenticates via the `CLAUDE_CODE_OAUTH_TOKEN`
  environment variable, not an on-disk credential, so a file-presence probe
  would report `NotFound` and **silently unschedule the one agent that
  currently works**. Any availability probe must be advisory (surfaced, not
  enforced) and is deferred.
- Coordinator-local execution fallback when no worker supports a profile. This
  contradicts the placement authority model.

## Requirements

### R1 — Worker validates its advertised profiles at startup

`WorkerConfig::from_env` parses each comma-separated entry as an optional
`EXECUTOR` or `EXECUTOR:VARIANT` pair and resolves the executor half through
`BaseCodingAgent::from_str`.

- Unknown executor name → startup error naming the offending value and listing
  the valid executor names. The worker must not register.
- Empty or unset variable → startup error. A worker eligible for nothing is a
  misconfiguration, not a default.
- Accepted entries are canonicalised to SCREAMING_SNAKE_CASE, preserving the
  variant verbatim (variants are user-defined and not enumerable).

Failing closed at startup is correct here: a worker that cannot be scheduled has
no useful degraded mode, and the deploy health gate will surface the failure.

### R2 — Matching is canonicalising, not byte-exact

`advertises_executor_profile` compares the executor half case-insensitively and
the variant half case-insensitively. R1 prevents new bad rows, but existing
registrations in the coordinator's database may already hold lowercase values
written by the current build; those must keep working across the upgrade without
an operator touching them.

The existing bare-vs-qualified semantics are preserved exactly: a bare
advertisement matches any variant of that executor; a qualified advertisement
pins one variant and is not widened by a bare request.

### R3 — The scheduling error states the cause and the remedy

`SchedulingError::NoEligibleWorkers` is replaced by two distinguishable
outcomes, decided by whether any worker passes the non-executor eligibility
checks (online, healthy mount, live lease):

- **At least one healthy worker, none advertising the executor** — report the
  requested profile plus the sorted set of profiles the healthy workers do
  advertise. Remedy: change agent, or advertise it on a worker.
- **No healthy worker at all** — report the worker count and a tally of the
  reasons they were rejected. Remedy: fix the workers. Naming the executor here
  would be actively misleading.

The distinction is derived from the existing `IneligibleReason` values, so no
new eligibility state is introduced.

### R4 — The create-mode picker does not offer unreachable executors

When worker nodes exist, `CreateChatBoxContainer` computes the union of
executor profiles advertised by *eligible* workers (online + healthy mount;
lease expiry is a server-side concern and is deliberately not re-implemented in
the browser) and marks any executor option outside that union as unsupported —
disabled, with a reason.

`WorkerNode.capabilities` is typed `unknown` in generated TypeScript
(`#[ts(type = "unknown")]`), so parsing must be defensive: any shape that is not
a string array yields no constraint. When the parsed union is empty — no
workers, capabilities absent, or an unrecognised shape — **no gating is applied
at all**. A UI that hides every agent because it failed to parse a field is
worse than the 400 it replaces.

This is an affordance, not an authorisation boundary; R3 remains the enforcement
point.

## Non-goals

- Changing placement, stickiness, dispatch, lease, or mount semantics.
- Changing the `cluster-protocol` wire format. `WorkerCapabilities` keeps
  `executor_profiles: Vec<String>`; only the values the worker puts in it become
  canonical. Registration and heartbeat payloads are covered by the request
  signature's body digest, so avoiding a shape change avoids a lockstep upgrade.
- A general worker capability framework.

## Verification

- Unit tests in `crates/worker` for R1: unknown executor rejected, empty
  rejected, mixed-case canonicalised, variant preserved.
- Unit tests in `crates/services` for R2 and R3: lowercase legacy advertisement
  matches a canonical request; bare/qualified semantics unchanged; the two
  error variants are produced in the right circumstances. Existing scheduler
  tests are updated to use data the running system can actually produce
  (canonical `CLAUDE_CODE`, requests like `CODEX:DEFAULT`) rather than the
  current lowercase fixtures.
- A frontend test for R4 covering: gating applied, unsupported option disabled,
  and each degenerate capability shape leaving the picker fully enabled.
- `pnpm run check`, `pnpm run lint`, `cargo test --workspace`,
  `pnpm run generate-types:check`, `pnpm run format`.
- Independent Codex review of the diff.

Per `wiki/self-hosted-deployment.md`, local tests do not replace the two-node
deployment gate. The R1 startup failure mode in particular should be confirmed
against a real worker before this is relied upon, because it converts a
misconfiguration that previously produced a running-but-useless worker into a
service that refuses to start.
