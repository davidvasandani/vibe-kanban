# Feature Specification: Legible Worker Executor Capabilities

**Feature dir**: `specs/vk/9f3d-cluster-worker-p/`
**Status**: Clarified — see [clarifications.md](clarifications.md) and
[analysis.md](analysis.md)

## Summary

On the clustered self-hosted deployment, choosing the Codex agent and sending a
prompt fails with `no eligible worker supports executor profile "CODEX:DEFAULT"`.
The scheduler is correct: both workers advertise only `CLAUDE_CODE`, so nothing
matches. What is wrong is everything around that correct decision — the operator
can advertise an executor name that does not exist or is cased wrong and get no
warning, the coordinator's rejection does not say what the cluster *can* run or
whether the problem is even the executor, and the UI offers agents no worker can
run so the user discovers this only after writing a prompt. This feature makes
an advertised capability validated where it is authored, tolerant of legacy
casing where it is consumed, self-explaining where it fails, and visible before
the user commits work.

It deliberately does **not** make Codex runnable on the workers; that requires
credentials this feature has no way to provision.

## User Stories

- As an operator, I want a worker to refuse to start when its advertised
  executor list is misspelled or empty, so that I find out at deploy time
  instead of discovering a silently unschedulable node weeks later.
- As an operator, I want a worker configured with `codex` to behave the same as
  one configured with `CODEX`, so that a casing choice in my Nix config is not a
  silent outage.
- As a user hitting a placement failure, I want the error to tell me whether no
  worker *supports* my agent or no worker is *healthy*, and what agents are
  available, so that I know whether to switch agents or go fix a node.
- As a user picking an agent, I want agents the cluster cannot run to be shown
  as unavailable up front, so that I do not write a prompt into a dead end.

## Functional Requirements

- **FR-1**: A worker validates each entry of its configured executor profile
  list at startup against the canonical set of known executors. An entry naming
  an unknown executor prevents startup, with an error naming the offending value
  and the valid names.
- **FR-2**: A worker whose configured list is absent, blank, or entirely
  whitespace prevents startup. Being capable of nothing is a misconfiguration,
  not a default.
- **FR-3**: Accepted entries are canonicalised before they are advertised. An
  entry may name a bare executor or an executor with a variant; both halves are
  canonicalised, each using the convention that already governs it — the
  executor half as the deserializer normalises executor names, the variant half
  as profile storage normalises variant keys.
  *(Revised during implementation. This originally said the variant was
  preserved verbatim, on the belief that variants are free-form. They are not:
  `canonical_variant_key` already imposes a canonical form that
  `ExecutorProfile` storage enforces, so a request always carries `PLAN` and an
  operator writing `codex:plan` would otherwise never match. See analysis I2.)*
- **FR-4**: Capability matching compares the executor half without regard to
  case or the `-`/`_` separator, so advertisements recorded by an earlier build
  continue to match after an upgrade with no operator action. Variant halves are
  compared exactly. This tolerance is permanent, not transitional: capabilities
  are written only by registration, never by heartbeat, so a stored
  non-canonical value survives for as long as that worker stays up.
- **FR-5**: Matching preserves existing bare-versus-qualified semantics: a bare
  advertisement satisfies any variant of that executor; a qualified
  advertisement satisfies only its own variant and is not widened by a bare
  request. A name that merely shares a prefix with an executor never matches.
  An advertisement the current build cannot resolve to a known executor keeps
  its present behaviour exactly, including the bare-prefix case.
- **FR-9**: When a user pins a specific worker that does not advertise the
  requested executor, the rejection names that worker's advertised profiles
  rather than reporting a bare reason code, and the worker picker marks workers
  that cannot run the current agent.
- **FR-6**: When placement finds no worker, the failure distinguishes two cases:
  at least one otherwise-healthy worker exists but none advertises the requested
  executor; or no worker is healthy at all. The first names the requested
  profile and the profiles that *are* available; the second reports how many
  workers were considered and why each was rejected, and does not blame the
  executor.
- **FR-7**: The agent picker in create mode marks as unavailable any agent that
  no currently-eligible worker advertises, showing a visible reason while
  leaving the agent listed. The comparison uses whole profiles, not just
  executor names, so an agent advertised only under a different variant is not
  reported as available. If the *currently selected* agent is unavailable the
  same reason is shown beside the picker; the selection is never changed
  automatically and submission is never blocked client-side.
- **FR-8**: FR-7's gate degrades to permitting every agent whenever the
  capability set cannot be determined — no workers known, capability data
  absent, or data in an unrecognised shape. Enforcement remains server-side.

## Out of Scope

- Provisioning Codex (or any other agent's) credentials on the worker nodes, and
  the deployment-repository changes that would accompany that. Blocked on an
  unresolved question about how Codex authenticates on the existing coordinator.
  **This feature does not make Codex work.**
- Probing for installed or authenticated agent CLIs to derive the advertised
  list automatically. The one agent that currently works on these workers
  authenticates by environment token rather than an on-disk credential, so a
  presence probe would withdraw it.
- Any change to placement, stickiness, dispatch, lease, or mount semantics.
- Any change to the coordinator/worker wire format.
- A coordinator-local execution fallback when no worker supports an agent.
- **Capability checking on follow-up messages.** Placement is scheduled exactly
  once, at workspace creation; follow-ups reuse the stored placement with no
  re-check, so switching agent mid-workspace can still reach a worker that never
  advertised it. The remedy is either re-placement — forbidden, because
  placement is sticky for a workspace's lifetime — or rejecting the follow-up,
  which is a product decision beyond this feature's mandate. The session-mode
  agent picker is likewise ungated. Recorded so the gap is known rather than
  assumed covered.

## Acceptance Criteria

- [ ] A worker configured with an unknown executor name exits at startup; the
      message contains the bad value and the list of valid names.
- [ ] A worker configured with an empty or whitespace-only list exits at
      startup, and that message also names the valid executors.
- [ ] An advertisement of `CODEX:` (empty variant) is normalised to `CODEX` by
      the worker, but a stored `CODEX:` row is *not* widened by the scheduler.
- [ ] A worker configured with `codex, claude-code` advertises `CODEX` and
      `CLAUDE_CODE`.
- [ ] A worker configured with `codex:PLAN` advertises `CODEX:PLAN` — variant
      casing unchanged.
- [ ] A worker record already holding a lowercase `codex` satisfies a request
      for `CODEX:DEFAULT` without the operator touching anything.
- [ ] An advertisement of `codexfoo` does not satisfy a request for
      `CODEX:DEFAULT`.
- [ ] An advertisement of `CODEX:PLAN` does not satisfy `CODEX:DEFAULT`, and a
      bare `CODEX` request does not widen it.
- [ ] With one healthy worker advertising only `CLAUDE_CODE`, requesting
      `CODEX:DEFAULT` yields an error naming `CODEX:DEFAULT` and listing
      `CLAUDE_CODE`.
- [ ] With every worker offline, requesting any profile yields an error
      reporting the worker count and rejection reasons, and does not name the
      executor as the cause.
- [ ] With no workers registered at all, the same not-healthy error is produced
      with a count of zero.
- [ ] With a mixed population — one offline worker, one healthy worker lacking
      the executor — the unsupported-executor error is produced, because that is
      the actionable remedy.
- [ ] In create mode against a cluster advertising only `CLAUDE_CODE`, the Codex
      option is shown disabled with a reason and Claude Code is selectable.
- [ ] In create mode, each of: no worker nodes; a worker with no capability
      data; capability data whose profile list is missing, not an array, or an
      array of non-strings — leaves every agent selectable.
- [ ] Workers that are offline or have an unhealthy mount contribute nothing to
      the set of agents the picker considers available. Lease expiry is
      deliberately *not* re-checked in the browser: it degrades open, and the
      coordinator's `expire_leases` marks such workers offline anyway.
- [ ] An agent advertised by a worker only as `CURSOR` is still offered, despite
      the picker listing it as `CURSOR_AGENT`.
- [ ] A cluster advertising only `CODEX:PLAN` does not offer Codex as available.
- [ ] Pinning a worker that lacks the requested executor yields an error naming
      that worker's advertised profiles, and that worker is marked in the worker
      picker.
- [ ] `cargo test --workspace`, `pnpm run check`, `pnpm run lint`, and
      `pnpm run generate-types:check` pass.
- [ ] A staged rollout is used: one worker upgraded and confirmed registering
      before the second, so a latent misconfiguration cannot take both nodes
      down at once under the new fail-closed startup.

## Clarifications

Resolved by `/speckit.clarify`; full reasoning in
[clarifications.md](clarifications.md).

1. **Fail-closed startup applies to both invalid and empty lists.** No live
   impact — both workers are already configured validly — and such a worker was
   never schedulable for new work. The error must name the valid set.
2. **An unsupported current selection is marked, never auto-switched, and never
   blocks submit.** Auto-switching would persist a changed default behind the
   user's back; blocking would make an affordance into an enforcement point.
3. **The error lists advertised profiles verbatim**, not collapsed to bare
   executor names — a consumer must not widen what its owner advertised, and the
   advertised string is the value the operator must edit.
4. **No deprecation path for non-canonical stored rows.** Originally justified as
   a transient upgrade window; that reasoning was wrong (capabilities are written
   only by registration, never heartbeat). Silent tolerance still stands, but
   FR-4 is permanent rather than transitional.
