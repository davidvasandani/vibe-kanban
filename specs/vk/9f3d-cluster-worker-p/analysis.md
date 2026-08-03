# Analysis: Legible Worker Executor Capabilities

`/speckit.analyze` cross-checked spec, clarifications, research, plan and tasks
against each other, against the constitution (v0.17.0), and against the source.
Nineteen findings; every one is dispositioned below. Four were independently
re-verified against the code before acceptance because they change the design.

## Blocker

### B1 — `CodingAgent::VARIANTS` is unreachable from `crates/worker` — **accepted**

`VARIANTS` is an associated const on the `strum::VariantNames` **trait**, which
must be in scope. `crates/worker/Cargo.toml` has no `strum` dependency, and
`executors` re-exports only `strum_macros` (the derive crate). `grep -rn
"VARIANTS" crates/ --include=*.rs` returns zero hits — the pattern is
unexercised anywhere in the repo. T020 as written would not compile.

**Fix**: L1 gains `pub fn valid_executor_names() -> String` in
`crates/executors/src/profile.rs`, which already owns the canonical-enumeration
concern and which the worker already depends on. Avoids adding a top-level
dependency (constitution *Constraints*).

## Major

### M2 — The byte-exact fallback drops the bare-prefix branch — **accepted**

The plan claimed the fallback made unresolvable names "behave exactly as they do
today". False: today's predicate is `a == r` **or** the bare branch, and the
fallback kept only the first. Regressions: advertised `claude` (not an executor
name) against requested `claude:DEFAULT` matches today, would not; likewise
`codexfoo` / `codexfoo:X`.

Live impact is nil — the only caller sends `Display` output — but the invariant
is what justifies the design.

**Fix**: the fallback becomes the *entire* current predicate, plus a test
pinning it.

### M3 — Collapsing an empty variant widens matching at the consumer — **accepted**

`"CODEX:"` today contains `':'`, so it is excluded from the bare branch and
matches only itself. Collapsing it to `(CODEX, None)` in the scheduler would make
it match every variant — a consumer widening an advertisement whose author wrote
a `':'`, which constitution XXII forbids and clarification 3 refuses elsewhere.

**Fix**: `canonical_profile_parts` becomes faithful — the variant is `Some(v)`
iff a `':'` was present, `v` possibly empty. Dropping an empty variant is
**authoring-side only** (`canonical_profile_string`, used by the worker). The
scheduler compares what it was given.

### M4 — The frontend gate does not know the `CURSOR` alias — **accepted**

A row advertising `CURSOR` (which `from_str` accepts) yields frontend set
`{CURSOR}` while `executorOptions` carries `CURSOR_AGENT`, so the picker would
disable an agent the scheduler *would* place — constitution XXII's "never used to
silently withdraw a capability that currently works".

**Fix**: alias map in the helper, with a test mirroring T011.

### M5 — The frontend gate collapses qualified advertisements — **accepted**

Taking only the executor half means a cluster advertising `CODEX:PLAN` shows
Codex as selectable, the user writes a prompt, and the server correctly rejects
`CODEX:DEFAULT`. That is the dead end FR-7 exists to prevent, via the same
collapse clarification 3 rejected for the error message.

**Fix**: the helper returns full profiles and mirrors
`advertises_executor_profile`'s wildcard semantics.

### M6 — `title` never renders on a `pointer-events-none` item — **accepted**

A native tooltip needs hover; `data-[disabled]:pointer-events-none` suppresses
it. FR-7's "with a reason" would not be delivered.

**Fix**: use the `badge?: React.ReactNode` prop that
`packages/ui/src/components/Dropdown.tsx:175` already exposes (verified), so the
reason is visible text.

### M7 — Acceptance criterion 12 had no test — **accepted**

Both FR-7 and FR-8 mapped to a pure-function test that cannot observe rendering.

**Fix**: new T045, a rendered-DOM test asserting the disabled state, the visible
reason, and that a supported option stays clickable.

### M8 — The manual-worker path stays opaque — **accepted**

`RequestedWorkerIneligible` formats `{reason:?}` — the bare string
`MissingExecutor`, naming nothing available. The worker `Select` disables only
offline/unhealthy workers, so pinning think3 with Codex reaches that error.

**Fix**: a `MissingExecutor` specialisation carrying that worker's advertised
profiles, and capability-aware disabling in the worker `Select`.

### M9 — Follow-ups re-use sticky placement with no capability re-check — **accepted as scope, documented**

`worker_scheduler()` has exactly one call site (workspace creation). Follow-ups
reuse the stored `WorkspacePlacement` with no re-check, so switching agent
mid-workspace reaches a worker that never advertised it — with neither the FR-6
error nor the FR-7 affordance. `SessionChatBox.tsx:20` also imports the
`ExecutorProps` type being widened and would silently ignore the new field.

**Disposition**: genuinely out of scope — the remedy is either re-placement
(forbidden: placement is sticky for a workspace's lifetime) or rejection, which
is a product decision this feature has no mandate for. Recorded in spec.md "Out
of Scope" with the reason, and in the plan as a known blast-radius note.

### M10 — spec.md still read as unclarified — **accepted**

Four `[NEEDS CLARIFICATION]` markers and `Status: Draft` after clarify ran.

**Fix**: decisions folded into a `## Clarifications` section, status flipped.

## Minor

- **m11 — strum picks the *longest* serialize value, not the last** — accepted;
  `research.md` R-2 corrected. Outcome coincides (`CURSOR_AGENT` is longer), but
  the stated rule was wrong and a future shorter-canonical alias would break
  silently. Also noted: `CodingAgent::VARIANTS` derives from the outer enum's
  `serialize_all`, so the two lists agreeing is a coincidence worth a test.
- **m12 — R-5 cited the wrong file** — accepted; `CreateChatBox.tsx:6` imports
  from `./Dropdown`, not `./DropdownMenu`. Conclusion survives, but T042 would
  have edited the wrong component.
- **m13 — clarification 4's "window measured in seconds" is wrong** — accepted,
  and independently verified: `WorkerHeartbeat`
  (`crates/cluster-protocol/src/lib.rs:80-85`) carries no capabilities, and
  `registry.rs:118` re-writes `current.capabilities`. Only `register` updates
  them, so a coordinator-only upgrade leaves stale rows for the workers' entire
  uptime — days. The decision (silent tolerance) stands, but FR-4 is load-bearing
  **indefinitely**, not transiently. Clarification 4 rewritten; the T061 KB page
  must not repeat the claim.
- **m14 — the empty-list error names neither the value nor the valid set** —
  accepted; a dedicated message replaces the bare `Missing` reuse.
- **m15 — case-insensitive variant comparison exceeds the spec** — accepted;
  variants now compare byte-exactly, matching FR-3's "preserved exactly as
  written" and removing an unrequested widening.
- **m16 — the rolling-upgrade hazard had no task** — accepted. Direction
  analysis: worker-first is safe; coordinator-first is safe *because of* FR-4.
  The uncovered hazard is a simultaneous two-node upgrade, where any node with an
  unset or misspelled variable now refuses to start and the cluster can go down
  at once. New T055 (config pre-flight, staged rollout) and an AC.
- **m17 — root `SPEC.md` / `IMPLEMENTATION_PLAN.md` / `PRIOR_KNOWLEDGE.md`
  duplicate the feature dir** — acknowledged, not fixed. They are this pipeline's
  per-task scratch artifacts and are regenerated per task; `specs/vk/` is
  canonical. Noted here so a future reader does not treat the root copies as
  authoritative.
- **m18 — more unrealistic fixtures** — accepted for
  `crates/worker/src/server.rs:410` (`vec!["codex".into()]`), same class as R-8.
  The empty-capability fixtures in `reconcile.rs` / `registry.rs` are left alone;
  they exercise registry plumbing, not matching.
- **m19 — the frontend gate ignores lease expiry** — accepted as documentation
  only. It degrades *open*, which is the correct direction, and `expire_leases`
  flips such workers to `offline` anyway. Recorded in an AC.

## Found during implementation and review (not by `/speckit.analyze`)

### I1 — The UI contradicted the backend on an exactly-advertised `DEFAULT` — **fixed**

`useExecutorConfig.ts:16` composes the request as
`` `${executor}:${variant ?? 'DEFAULT'}` ``. A worker advertising exactly
`CODEX:DEFAULT` therefore *is* satisfiable, but the container called
`clusterSupportsExecutor` without a variant, and the helper treated a missing
variant as "no variant" — answering `false` and greying out a working agent.
The same class of defect as M4, introduced while fixing M5.

Fix: an omitted variant now means "any variant of this executor" (degrade open),
and the container passes the variant it will actually send for the current
selection. Two regression tests added.

### I2 — FR-3's "variants are preserved verbatim" premise was wrong — **spec superseded**

The spec asserted variants are free-form and must not be case-folded, and the
analysis (m15) reinforced it. Reading `profile.rs` showed the opposite: variant
keys already have a canonical form (`canonical_variant_key` — SCREAMING_SNAKE
with `DEFAULT` preserved) that `ExecutorProfile` storage enforces, so a request
always carries `PLAN`, never `plan`. Byte-exact variant comparison would have
left `codex:plan` in Nix permanently unmatchable — the exact bug being fixed,
one level down.

Resolution: reuse the shipped `canonical_variant_key` on both sides
(constitution VI). This *supersedes* m15.

### I3 — `cargo test --workspace` false pass — **corrected**

An earlier verification piped cargo through `tail`, so the exit code read was
`tail`'s. It hid that `crates/tauri-app` cannot build here (no GTK/glib). Re-run
unpiped as `--exclude vibe-kanban-tauri`: exit 0, 70 binaries, 0 failures.

### I4 — Exhaustive parity test added

Rather than reasoning about M2's truth table by inspection, the pre-change
predicate is kept in the test module and asserted equivalent across 135
(advertised, requested) pairs except an explicit list of 9 intended fixes. This
is now the main evidence that the canonicalisation rewrite changed nothing
unintended.

### I5 — Constitution XIX collided with main; slimmed and renumbered to XXII

While this branch was in flight, main added principles XIX (Observability), XX
(Cross-node paths) and XXI (One convention per concept), reaching v0.19.0. The
proposed principle collided by number *and* substantially by content: XXI
already requires reusing an existing resolution rule rather than re-deriving it,
and already states that *"a consumer that handles every case except the default
is broken for almost every user"* — which is exactly defect I1 above, found
independently in this branch's own UI code.

Resolution: keep main's XIX–XXI untouched, renumber to **XXII**, and cut
everything XXI already says. What remains is only the genuinely new material:
validate an operator-supplied capability list at its owner and fail closed; a
consumer must not widen what it was given; probes stay advisory and never
withdraw a working capability; a UI mirror degrades to permitting everything.
Constitution bumped to v0.20.0 and all references in this feature's documents
updated from XIX to XXII.

## Net effect

No finding invalidates the approach. Three change observable behaviour (M3, m15
narrow matching; M5 widens what the UI checks), one adds a task layer (M7, M8),
one removes a false premise from the rationale (m13), and one prevents a
non-compiling task (B1). Task count 27 → 31.
