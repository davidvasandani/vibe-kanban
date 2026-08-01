# Technical Plan: Legible Worker Executor Capabilities

**Feature dir**: `specs/vk/9f3d-cluster-worker-p/`
**Spec**: [spec.md](spec.md) · **Clarifications**: [clarifications.md](clarifications.md)
· **Research**: [research.md](research.md)

## Approach

Four thin changes at three existing layers, no new abstraction and no wire
change. The capability string stays a `String` on the protocol; what changes is
that the component *authoring* it validates and canonicalises it (FR-1..3), the
component *consuming* it compares canonically (FR-4, FR-5), the failure explains
itself (FR-6), and the picker reflects it (FR-7, FR-8).

One shared parsing helper in `crates/executors` serves both the worker and the
scheduler, which already depend on that crate. This is the only new code that is
not a modification of an existing function.

## Layers

### L1 — `crates/executors/src/profile.rs`: canonical parsing

Two free functions beside `ExecutorProfileId`:

```
pub fn canonical_profile_parts(raw: &str) -> Option<(BaseCodingAgent, Option<&str>)>
pub fn canonical_profile_string(raw: &str) -> Option<String>
pub fn valid_executor_names() -> String
```

`canonical_profile_parts` splits on the first `':'`, applies the existing
kebab→SCREAMING_SNAKE normalisation (`replace('-', "_").to_ascii_uppercase()`,
matching `de_base_coding_agent_kebab` per research R-3), resolves the executor
half via `BaseCodingAgent::from_str`, and returns the variant slice unchanged.
`None` on an unknown executor or an empty executor half.

The returned variant is `Some(v)` **iff a `':'` was present**, where `v` may be
empty. This faithfulness matters: per analysis M3, collapsing `"CODEX:"` to "no
variant" inside the *scheduler* would let a consumer widen an advertisement
whose author wrote a `':'`, which constitution XIX forbids.

`canonical_profile_string` renders `EXECUTOR` or `EXECUTOR:VARIANT` with the
variant **verbatim** (FR-3), and **drops an empty variant** — this normalisation
is authoring-side only, applied by the worker to its own configuration.

`valid_executor_names` joins `CodingAgent::VARIANTS` for the FR-1 error message.
It lives here, not in the worker, because `VARIANTS` needs the `strum`
`VariantNames` trait in scope and `crates/worker` has no `strum` dependency
(analysis B1).

All halves are trimmed of surrounding whitespace.

### L2 — `crates/worker/src/lib.rs`: validate at startup (FR-1, FR-2)

Two new error variants:

```
#[error("invalid {name}: {value:?} is not a known executor (valid: {valid})")]
UnknownExecutorProfile { name: &'static str, value: String, valid: String }

#[error("{name} is required and must name at least one executor (valid: {valid})")]
NoExecutorProfiles { name: &'static str, valid: String }
```

`valid` comes from `valid_executor_names()` in L1 (analysis B1).

The empty case gets its own variant rather than reusing
`WorkerConfigError::Missing`: clarification 1 requires every fail-closed message
to name the valid set, and `Missing` renders only
`"VK_WORKER_EXECUTOR_PROFILES is required"` (analysis m14).

In `from_lookup`, the existing chain becomes: split on `','`, trim, drop empties,
map through `canonical_profile_string`, and collect — returning
`UnknownExecutorProfile` on the first entry that fails and `NoExecutorProfiles`
when the result is empty.

`WorkerConfigError` derives `PartialEq, Eq`; the new variant holds only owned
`String`s and a `&'static str`, so that still derives.

### L3 — `crates/services/.../cluster/scheduler.rs`: match and report

**Matching (FR-4, FR-5).** `advertises_executor_profile` becomes: canonicalise
both sides via `canonical_profile_parts`; if either fails, fall back to **the
entire current predicate** — `a == r || (!a.contains(':') && r.split_once(':')
.is_some_and(|(e, _)| e == a))`. Analysis M2 caught that falling back to only
`a == r` would silently drop the bare-prefix branch, regressing e.g. advertised
`claude` against requested `claude:DEFAULT`. Unreachable from today's single
caller, but the preservation claim is what justifies the design.

On success, executors must be equal, then either the advertisement has no
variant (matches anything, including a bare request) or both have variants
compared **byte-exactly**. Analysis m15: FR-3 says variants are preserved
exactly and FR-4 scopes case-insensitivity to the executor half, so
`eq_ignore_ascii_case` on variants would be an unrequested widening.

Prefix overlap stays excluded for free: `codexfoo` fails to resolve, so it takes
the fallback path and does not match `CODEX:DEFAULT`.

The existing block comment above this function records a production incident and
must be kept; extend it with the case-folding rationale rather than replacing it.

**Reporting (FR-6).** Replace `NoEligibleWorkers` with:

```
ExecutorUnsupported { executor_profile: String, supported: Vec<String> }
NoHealthyWorkers   { total: usize, reasons: Vec<(IneligibleReason, usize)> }
```

In `select`, when the filter yields nothing, classify every worker once with
`eligibility(...)`. If any worker failed *only* with `MissingExecutor`, that
worker is healthy and simply lacks the agent → `ExecutorUnsupported`, whose
`supported` is the sorted deduplicated union of those workers' advertised
strings, verbatim (clarification 3). Otherwise → `NoHealthyWorkers` with a tally
grouped by reason. Zero registered workers is `NoHealthyWorkers { total: 0,
reasons: [] }`.

`IneligibleReason` gains a `fn label(self) -> &'static str` for the tally text.
It is already `Copy + Eq`.

**Manual placement (FR-9).** `RequestedWorkerIneligible` currently formats
`{reason:?}`, so pinning a worker that lacks the agent yields the bare string
`MissingExecutor` and names nothing available (analysis M8). Add a
`RequestedWorkerMissingExecutor { worker_node_id, executor_profile, supported }`
variant, returned when the pinned worker's only failure is `MissingExecutor`.
Constitution XIX requires the capability failure to explain itself regardless of
which path reached it.

Per research R-4 the server call site needs no change and no generated type
moves.

### L4 — Frontend (FR-7, FR-8)

**`packages/web-core/src/shared/lib/workerCapabilities.ts`** — a pure function:

```
clusterAdvertisedProfiles(workers: WorkerNode[]): string[] | null
clusterSupportsExecutor(advertised: string[] | null, executor: string): boolean
```

`clusterAdvertisedProfiles` considers only `status === online && mount_status
=== healthy`, reads `capabilities.executor_profiles` accepting only a genuine
array of strings, and returns the raw advertised strings. It returns `null` —
meaning *no opinion, gate nothing* — when there are no workers or nothing
parses. Returning `null` rather than `[]` is the mechanism behind FR-8: an empty
result would disable every agent, the exact failure the requirement prevents.

`clusterSupportsExecutor` mirrors the backend predicate rather than collapsing
to executor names. Two corrections from analysis:

- **M5**: taking only the executor half would show Codex as available on a
  cluster advertising only `CODEX:PLAN`, then let the server reject
  `CODEX:DEFAULT` — recreating the dead end FR-7 exists to remove. Compare whole
  profiles with the same wildcard semantics (`bare advertisement` matches any
  variant).
- **M4**: canonicalisation must apply the `CURSOR` → `CURSOR_AGENT` alias. A row
  advertising `CURSOR` is legitimate, but `executorOptions` carries
  `CURSOR_AGENT`, so without the alias the picker would disable an agent the
  scheduler *would* place — constitution XIX's "never silently withdraw a
  capability that currently works".

Lease expiry is deliberately not re-checked client-side (analysis m19): it
degrades open, and `expire_leases` marks such workers offline anyway.

**`packages/ui/src/components/CreateChatBox.tsx`** — widen `ExecutorProps` with
optional `unsupported?: ReadonlyMap<TExecutor, string>`; pass `disabled` and
render the reason through the existing `badge` prop of the `DropdownMenuItem`
imported from `./Dropdown` (research R-5 as corrected: the file is `Dropdown.tsx`,
and a `title` tooltip cannot work under `pointer-events-none`). Optional, so
remote-web and `SessionChatBox` — the type's second consumer — compile untouched
(constitution IV).

**`CreateChatBoxContainer.tsx`** — derive from the already-fetched `workerNodes`
query (no new request), build the `unsupported` map over `executorOptions`,
render an inline reason beside the picker when the *current* selection is
unsupported, and additionally disable worker options in the "Run on" `Select`
that cannot run the current agent (FR-9). Per clarification 2: no auto-switch,
no write through `setExecutorOverrides`, submit stays enabled.

New i18n keys under `createMode.worker` in
`packages/web-core/src/i18n/locales/en/common.json`.

## Testing

| Requirement | Test | Location |
|---|---|---|
| L1 canonicalisation, alias, rejection | unit | `crates/executors/src/profile.rs` |
| FR-1, FR-2, FR-3 | unit | `crates/worker/src/lib.rs` |
| FR-4, FR-5 | unit | `crates/services/.../scheduler.rs` |
| FR-6 (4 populations) | unit | `crates/services/.../scheduler.rs` |
| FR-8 (degenerate shapes) | vitest | `packages/web-core/src/shared/lib/workerCapabilities.test.ts` |
| FR-7 (rendered picker) | vitest DOM | `packages/web-core/src/shared/components/CreateChatBoxContainer.test.tsx` |
| FR-9 | unit + vitest DOM | scheduler + container |

Analysis M7: FR-7 previously mapped only to the pure-function test, which cannot
observe a disabled option or a rendered reason, leaving acceptance criterion 12
unverifiable and violating constitution II.

Two existing tests must change, and both changes are themselves evidence:
`parses_required_identity_and_coordinator_with_safe_defaults` (research R-7) and
the scheduler fixtures (research R-8). The scheduler fixtures must move to
canonical values **before** the legacy-tolerance test lands, or that test proves
nothing.

## Risks

- **Fail-closed startup is the only behaviour change visible to operators**, and
  a simultaneous two-node upgrade could take the whole cluster down at once if
  either node carries a latent misconfiguration (analysis m16). Mitigated by
  clarification 1 (both live workers are already valid and canonical), by the
  error naming the valid set, and by requiring a **staged rollout** — upgrade
  one worker, confirm it registers, then the second. Still needs the two-node
  deployment check from `wiki/self-hosted-deployment.md`; local tests do not
  substitute.
- **Follow-ups remain ungated** (analysis M9). Placement is scheduled once, and
  follow-ups reuse it with no capability re-check, so switching agent
  mid-workspace can still reach a worker that never advertised it. Recorded in
  spec.md "Out of Scope" rather than silently left as an assumed-covered case.
- **Widening a `@vibe/ui` primitive touches both frontends.** Mitigated by
  making the prop optional and additive.
- **Feature-gated `QaMock` makes `CodingAgent::VARIANTS` vary** (research R-1).
  The message-content test must assert a stable substring, not the whole list.

## Contracts

No API, database, or wire-format contract changes. `SchedulingError` is internal
and not a ts-rs type; `WorkerCapabilities.executor_profiles` keeps its
`Vec<String>` shape. `pnpm run generate-types:check` is therefore a guard
against accidental export, and an empty diff is the expected result. No
`contracts/` directory is created for this feature.
