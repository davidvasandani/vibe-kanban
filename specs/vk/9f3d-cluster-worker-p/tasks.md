# Tasks: Legible Worker Executor Capabilities

Dependency-ordered. `[P]` = safe to do in parallel with the other `[P]` tasks in
the same layer (disjoint files, no shared symbols).

Layer 1 must land before layers 2 and 3 (both import its helper). Layers 2, 3
and 4 are mutually independent. Layer 5 is verification.

## Layer 0 — Setup

- [x] **T001** `pnpm install --frozen-lockfile` in the worktree.
- [x] **T002** Baseline: `cargo test -p executors -p worker -p services` green
      before any edit, so later failures are attributable.

## Layer 1 — Shared canonical parsing (blocks L2, L3)

- [x] **T010** Add `canonical_profile_parts`, `canonical_profile_string` and
      `valid_executor_names` to `crates/executors/src/profile.rs`. Split on
      first `':'`, trim both halves, apply
      `replace('-', "_").to_ascii_uppercase()` to the executor half, resolve via
      `BaseCodingAgent::from_str`. The returned variant is `Some(v)` **iff a
      `':'` was present** (`v` may be empty) — do **not** collapse it here
      (analysis M3). Only `canonical_profile_string` drops an empty variant,
      because that normalisation is authoring-side.
      `valid_executor_names` lives here, not in the worker, because `VARIANTS`
      needs the `strum::VariantNames` trait in scope and `crates/worker` has no
      `strum` dependency (analysis B1).
- [x] **T011** Unit tests in the same file: `codex`→`CODEX`;
      `claude-code:PLAN`→`CLAUDE_CODE:PLAN`; `codex:plan` keeps lowercase
      `plan`; `nope`→`None`; `""`→`None`; whitespace tolerated.
      `canonical_profile_string("CODEX:")`→`"CODEX"` while
      `canonical_profile_parts("CODEX:")` yields `Some("")` — the asymmetry is
      the point. **`CURSOR`→`CURSOR_AGENT`** asserted explicitly, plus an
      assertion that `valid_executor_names()` contains `CURSOR_AGENT` (research
      R-2 as corrected: strum picks the *longest* serialize value, and the two
      lists agreeing is a coincidence, not a guarantee).

## Layer 2 — Worker startup validation (FR-1, FR-2, FR-3)

- [x] **T020** Add `WorkerConfigError::UnknownExecutorProfile { name, value,
      valid }` **and** `NoExecutorProfiles { name, valid }` to
      `crates/worker/src/lib.rs`, both sourcing `valid` from
      `valid_executor_names()` (analysis B1 — `CodingAgent::VARIANTS` is not
      reachable from this crate). The empty case needs its own variant because
      reusing `Missing` would print a message naming neither the value nor the
      valid set, which clarification 1 forbids (analysis m14).
- [x] **T021** Rewrite the `executor_profiles` parse in `from_lookup` to map
      entries through `canonical_profile_string`, failing on the first unknown
      entry and returning `NoExecutorProfiles` for an empty result.
- [x] **T022** Update `parses_required_identity_and_coordinator_with_safe_defaults`
      to supply the now-required variable and assert the canonical result
      (research R-7). This test previously encoded the defect.
- [x] **T023** [P] New tests: unknown name rejected with the value in the
      message; empty and whitespace-only rejected; `codex, claude-code` →
      `["CODEX","CLAUDE_CODE"]`; `codex:PLAN` → `["CODEX:PLAN"]`. Assert a
      stable substring of the valid-names list, not the whole list
      (`QaMock` is feature-gated — research R-1).

## Layer 3 — Scheduler matching and reporting (FR-4, FR-5, FR-6)

- [x] **T030** Rewrite `advertises_executor_profile` in
      `crates/services/src/services/cluster/scheduler.rs` to canonicalise both
      sides. When either side fails to resolve, fall back to the **entire**
      current predicate — `a == r || (!a.contains(':') && r.split_once(':')
      .is_some_and(|(e, _)| e == a))` — not just `a == r`, which would drop the
      bare-prefix branch and regress e.g. advertised `claude` vs requested
      `claude:DEFAULT` (analysis M2). Compare variants **byte-exactly**, not
      case-insensitively (analysis m15). Preserve the existing block comment and
      extend it with the case-folding rationale.
- [x] **T030b** Tests pinning the fallback: an unresolvable bare advertisement
      still matches a qualified request for the same unknown name; a stored
      `CODEX:` is *not* widened to match `CODEX:DEFAULT` (analysis M3); variant
      case differences do not match.
- [x] **T031** Migrate the existing test fixtures from `["codex","claude"]` and
      bare lowercase requests to canonical `["CODEX","CLAUDE_CODE"]` and
      requests like `CODEX:DEFAULT` (research R-8). **Must precede T032** — with
      the old fixtures a legacy-tolerance test would be indistinguishable from
      the default path.
- [x] **T032** Add the legacy-tolerance regression test: a stored lowercase
      `codex` advertisement satisfies `CODEX:DEFAULT`. Keep the `codexfoo`
      prefix-overlap test and the qualified-pinning tests passing unchanged in
      meaning.
- [x] **T033** Add `IneligibleReason::label()`; replace `NoEligibleWorkers` with
      `ExecutorUnsupported { executor_profile, supported }` and
      `NoHealthyWorkers { total, reasons }`, with `Display` text that states the
      remedy.
- [x] **T034** Implement the classification in `select`: on empty filter,
      evaluate `eligibility` once per worker; any worker failing *only* with
      `MissingExecutor` ⇒ `ExecutorUnsupported` with the sorted deduplicated
      union of those workers' advertised strings **verbatim** (clarification 3);
      otherwise `NoHealthyWorkers` with a per-reason tally.
- [x] **T035** [P] Tests for all four populations: healthy-but-unsupported;
      all-offline; zero workers (`total: 0`); mixed offline + healthy-without-
      executor ⇒ prefers `ExecutorUnsupported`.
- [x] **T036** Confirm no other call site matches the removed variant by name
      (research R-4 says there is none) and that
      `crates/server/.../workspaces/create.rs` still compiles untouched.
- [x] **T037** FR-9: add `RequestedWorkerMissingExecutor { worker_node_id,
      executor_profile, supported }`, returned when a *pinned* worker's only
      failure is `MissingExecutor`. Today that path formats `{reason:?}` and
      emits the bare string `MissingExecutor`, naming nothing available
      (analysis M8). Test it.
- [x] **T038** [P] Migrate the `executor_profiles: vec!["codex".into()]` fixture
      at `crates/worker/src/server.rs:410` to a canonical value — same
      unrealistic-fixture class as R-8 (analysis m18).

## Layer 4 — Frontend gating (FR-7, FR-8)

- [x] **T040** [P] Add `clusterAdvertisedProfiles` and `clusterSupportsExecutor`
      to `packages/web-core/src/shared/lib/workerCapabilities.ts`. Online +
      healthy-mount workers only; defensive parse of `capabilities`; return
      `null` for "gate nothing". `clusterSupportsExecutor` must compare **whole
      profiles** with the backend's wildcard semantics, not just executor halves
      (analysis M5 — otherwise a `CODEX:PLAN`-only cluster shows Codex as
      available and the server rejects it), and must apply the `CURSOR` →
      `CURSOR_AGENT` alias (analysis M4 — otherwise the picker withdraws an
      agent that actually works).
- [x] **T041** [P] Colocated vitest: normal cluster gates correctly; offline and
      unhealthy-mount workers excluded; a `CODEX:PLAN`-only cluster does **not**
      report Codex supported; a `CURSOR` advertisement **does** support
      `CURSOR_AGENT`; and every degenerate shape (no workers, no
      `capabilities`, missing `executor_profiles`, non-array, array of
      non-strings) returns `null`. The degenerate cases are FR-8 and are the
      point of the helper.
- [x] **T042** Widen `ExecutorProps` in `packages/ui/src/components/CreateChatBox.tsx`
      with optional `unsupported?: ReadonlyMap<TExecutor, string>`; pass
      `disabled` and render the reason via the existing **`badge`** prop of the
      `DropdownMenuItem` imported from **`./Dropdown`** — not `./DropdownMenu`
      (analysis m12), and not a `title` tooltip, which cannot render under
      `data-[disabled]:pointer-events-none` (analysis M6). Optional keeps
      remote-web *and* `SessionChatBox` — the type's second consumer — compiling
      untouched.
- [x] **T043** Wire `CreateChatBoxContainer.tsx`: derive from the existing
      `workerNodes` query, build the map, render an inline reason when the
      *current* selection is unsupported, and disable "Run on" worker options
      that cannot run the current agent (FR-9). No auto-switch, no
      `setExecutorOverrides` write, submit stays enabled (clarification 2).
- [x] **T044** Add i18n keys under `createMode.worker` in
      `packages/web-core/src/i18n/locales/en/common.json`.
- [x] **T045** Rendered-DOM vitest for FR-7 (analysis M7 — acceptance criterion
      12 is otherwise unverifiable): the unsupported agent renders disabled with
      its reason visible, a supported agent stays clickable, and an unsupported
      *current selection* shows the inline notice without changing the
      selection.

## Layer 5 — Verification

- [x] **T050** `cargo test -p executors -p worker -p services`, then
      `cargo test --workspace`.
- [x] **T051** [P] `pnpm run generate-types:check` — expect **no** diff;
      `SchedulingError` is not a ts-rs type (research R-4). A diff here means
      something leaked into the generated contract.
- [x] **T052** [P] `pnpm run check` and `pnpm run lint`.
- [x] **T053** `pnpm run format` (repository requirement, run last).
- [x] **T054** Read the complete diff for consistency. Note: this worktree's
      `.git` points at an unreachable gitdir, so `git diff` is unavailable —
      review file-by-file instead.
- [ ] **T055** Document the staged-rollout requirement for this change (analysis
      m16): upgrade one worker, confirm it registers, then the second. Under the
      new fail-closed startup a simultaneous two-node deploy can take the whole
      cluster down at once if either node carries a latent misconfiguration.
      Worker-first ordering is safe; coordinator-first is safe only because of
      FR-4.

## Layer 6 — Review and knowledge capture

- [ ] **T060** Independent Codex review of the diff; iterate to no significant
      findings.
- [ ] **T061** Add `docs/knowledge-base/worker-capability-advertisement.md` (no
      such page exists) following that directory's conventions: `Tags:` line
      under the H1, a `## Verification pattern` section, and a table row
      appended to `INDEX.md` with link text excluding `.md`. Cross-reference
      `clustered-workspace-execution.md` and `wiki/self-hosted-deployment.md`.

## Not in this feature

Codex enablement on think3/think4 (homelab module + host config + credential
provisioning). Blocked on the Codex auth-shape question. **Completing every task
above still leaves Codex unrunnable** — it makes the refusal explain itself and
prevents the silent-misconfiguration class.
