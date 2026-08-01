# Implementation Plan: Legible Worker Executor Capabilities

Implements `SPEC.md` R1–R4. Ordered so each step compiles and tests green on its
own; the two Rust layers are independent of each other and of the frontend.

## Step 0 — Environment

1. `pnpm install --frozen-lockfile` (fresh worktree requirement).
2. Confirm baseline: `cargo test -p worker -p services`.

Note for this workspace: the checkout's `.git` points at
`/srv/src/vibe-kanban/.git/worktrees/vibe-kanban4`, which is not reachable from
this sandbox, so `git`-based verification (`git diff --check`, diff review
against a base ref) is unavailable and must be substituted with file-level
review.

## Step 1 — Shared profile parsing helper (`crates/executors/src/profile.rs`)

Both the worker (R1) and the scheduler (R2) need the same notion of "split an
`EXECUTOR[:VARIANT]` string and canonicalise the executor half". Put it once,
next to `ExecutorProfileId`, rather than duplicating a `split_once(':')` in two
crates that already both depend on `executors`.

1. Add `pub fn canonical_profile_parts(raw: &str) -> Option<(BaseCodingAgent,
   Option<&str>)>`: split on the first `':'`, uppercase-and-underscore the
   executor half (mirroring the existing `de_base_coding_agent_kebab`
   normalisation so `claude-code` and `claude_code` both resolve), resolve via
   `BaseCodingAgent::from_str`, return `None` on an unknown executor.
2. Add `pub fn canonical_profile_string(raw: &str) -> Option<String>` returning
   `EXECUTOR` or `EXECUTOR:VARIANT` with the executor half canonical and the
   variant **preserved verbatim** — variants are user-defined and not
   enumerable, so they must not be case-folded on the way in.
3. Unit tests: `codex` → `CODEX`; `claude-code:PLAN` → `CLAUDE_CODE:PLAN`;
   `codex:plan` keeps `plan`; `nope` → `None`; `CURSOR` → `CURSOR_AGENT` (the
   alias path); empty → `None`.

Risk: `BaseCodingAgent::from_str` is strum-derived with `serialize_all =
"SCREAMING_SNAKE_CASE"` and an explicit `CURSOR`/`CURSOR_AGENT` alias. Verify
the alias resolves through `from_str` and not only through serde before relying
on it in a test.

## Step 2 — R1: worker validates at startup (`crates/worker/src/lib.rs`)

1. Add `WorkerConfigError::UnknownExecutorProfile { value: String, valid:
   String }` with a message naming the bad value and listing valid executor
   names from `BaseCodingAgent::VARIANTS` (the `VariantNames` derive is already
   present on `CodingAgent`; confirm the discriminant type also exposes it, and
   if not, source the list from strum's `VariantNames` on the discriminants).
2. In `from_lookup`, replace the current `unwrap_or_default().split(',')` chain:
   map each non-empty trimmed entry through `canonical_profile_string`,
   collecting the first failure into the new error.
3. Reject an empty resulting list with `WorkerConfigError::Missing(
   WORKER_EXECUTOR_PROFILES_ENV)` — unset, blank, and all-whitespace collapse to
   the same operator-visible mistake.
4. **Update the existing test** `parses_required_identity_and_coordinator_with_
   safe_defaults`, which currently omits `VK_WORKER_EXECUTOR_PROFILES` and will
   now fail. This is intended: the test documented a default that produced a
   worker eligible for nothing.
5. New tests: unknown name rejected; empty/whitespace rejected; `codex,
   claude-code` canonicalises to `["CODEX", "CLAUDE_CODE"]`; `codex:PLAN`
   preserves the variant; surrounding whitespace tolerated.

## Step 3 — R2: canonicalising match (`crates/services/.../scheduler.rs`)

1. Rewrite `advertises_executor_profile` to compare via
   `canonical_profile_parts` on both sides: executors must be the same
   `BaseCodingAgent`; then either the advertisement has no variant (matches any
   variant of that executor, including a bare request) or both variants are
   present and `eq_ignore_ascii_case`.
2. Fall back to the current byte-exact comparison when **either** side fails to
   canonicalise, so an advertisement for an executor this build does not know
   still behaves as it does today rather than becoming unmatchable.
3. Preserve the documented semantics exactly, and keep the existing explanatory
   comment (it records why bare advertisements match qualified requests — a
   production incident). Extend it to record the case-folding rationale.
4. Update the existing fixtures from lowercase `"codex"`/`"claude"` to values
   the system actually produces (`CODEX`, `CLAUDE_CODE`) and requests of the
   form `CODEX:DEFAULT`, then **add** a regression test that a legacy lowercase
   row still matches — that is the upgrade-compatibility guarantee, and it is
   only meaningful once the rest of the fixtures are realistic.
5. Keep the prefix-overlap test (`codexfoo`) — canonicalisation must not
   reintroduce that false match; `codexfoo` simply fails to resolve and falls to
   the byte-exact path.

## Step 4 — R3: cause-distinguishing error (same file, then call site)

1. Replace `SchedulingError::NoEligibleWorkers { executor_profile }` with:
   - `ExecutorUnsupported { executor_profile: String, supported: Vec<String> }`
   - `NoHealthyWorkers { total: usize, reasons: Vec<(IneligibleReason, usize)> }`
   Derive `Display` messages that state the remedy, not just the fact.
2. In `select`, when the filtered iterator is empty, partition the workers once
   by `eligibility(...)`: if any worker's only failure is `MissingExecutor`,
   emit `ExecutorUnsupported` with the sorted deduplicated union of those
   workers' advertised profiles; otherwise emit `NoHealthyWorkers` with a tally.
   Zero workers registered is `NoHealthyWorkers { total: 0, .. }`.
3. `IneligibleReason` needs `Display` (or a `fn label()`) for the tally text; it
   is `Copy + Eq` already.
4. Call site `crates/server/src/routes/workspaces/create.rs:383` maps the error
   with `ApiError::BadRequest(error.to_string())` and needs no change — but
   re-read it to confirm no variant is matched by name anywhere. Grep for
   `NoEligibleWorkers` across the workspace first; it is currently referenced
   only in `scheduler.rs`.
5. Tests: healthy-but-unsupported produces `ExecutorUnsupported` listing the
   real profiles; all-offline produces `NoHealthyWorkers` with a correct tally;
   an empty worker list produces `NoHealthyWorkers { total: 0 }`; a mixed
   population (one offline, one healthy-without-the-executor) prefers
   `ExecutorUnsupported`, because the actionable remedy is the executor.

## Step 5 — R4: create-mode gating (`packages/web-core`)

1. New pure helper + colocated Vitest file, e.g.
   `shared/lib/workerCapabilities.ts`:
   `clusterSupportedExecutors(workerNodes): Set<string> | null`.
   - Consider only workers with `status === online && mount_status === healthy`.
   - Parse `capabilities` defensively — it is `unknown` in generated types.
     Anything that is not `{ executor_profiles: string[] }` contributes nothing.
   - Return the canonical executor halves (uppercase, before any `':'`).
   - Return **`null`** (meaning "no opinion, gate nothing") when there are no
     worker nodes at all or the union is empty. Encoding this as `null` rather
     than an empty set is what prevents a parse failure from disabling every
     agent.
2. In `CreateChatBoxContainer`, derive the set from the already-fetched
   `workerNodes` query and pass a disabled/reason flag through to the executor
   options given to `CreateChatBox`.
3. Inspect how `executorOptions` from `useExecutorConfig` is consumed by
   `CreateChatBox` before choosing between filtering the list and adding a
   `disabled` field. **Prefer disabling with a reason** — silently omitting an
   agent the user has configured is its own debugging dead end, and the KB's
   "unsupported ≠ error" guidance argues for a visible status.
4. Vitest cases: gating applied for a normal cluster; unsupported option
   disabled; offline/unhealthy workers excluded from the union; and each
   degenerate shape (no workers, missing `capabilities`, `executor_profiles`
   absent, non-array, array of non-strings) leaves the picker fully enabled.

## Step 6 — Verification

1. `cargo test -p executors -p worker -p services`, then
   `cargo test --workspace`.
2. `pnpm run generate-types:check` — no Rust type crossing the ts-rs boundary
   should change (`SchedulingError` is not a `TS` type), so this is a guard
   against an unintended export, not an expected diff.
3. `pnpm run check`, `pnpm run lint`.
4. `pnpm run format` last.
5. Re-read the full diff for consistency before review.

## Step 7 — Review and knowledge capture

1. Independent Codex review of the diff; iterate to no significant findings.
2. New `docs/knowledge-base/worker-capability-advertisement.md` — the KB has no
   page on this — following that directory's conventions (`Tags:` line under the
   H1, `## Verification pattern` section, table row appended to `INDEX.md` with
   link text excluding `.md`). Cross-reference
   `wiki/self-hosted-deployment.md` and `clustered-workspace-execution.md`.

## Deliberately not done

- Codex enablement on think3/think4 (homelab `modules/vibe-kanban-rebuild.nix`
  plus `hosts/think/think{3,4}.nix`). Blocked on the Codex auth shape; see
  `SPEC.md` "out of scope". **This plan does not make Codex runnable** — it makes
  the failure explain itself and prevents the silent-misconfiguration class.
- Availability probing to auto-derive profiles; would unschedule Claude Code on
  these workers, per `PRIOR_KNOWLEDGE.md` guidance 8.
