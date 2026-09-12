# Analysis: Gmail MCP connector (`vk/4daf-gmail-mcp`)

Cross-check of `spec.md`, `plan.md`, `tasks.md`, `research.md`,
`contracts/README.md` and the root `SPEC.md` / `IMPLEMENTATION_PLAN.md` against
`.specify/memory/constitution.md` v0.20.0.

**Verdict: 1 error, 4 warnings, 6 info.** The error is a wrong command string in
a companion document and is corrected below. No constitution violations. Nothing
blocks implementation.

---

## Errors

### E1 — Wrong pnpm filter in `IMPLEMENTATION_PLAN.md` (root)

**Artifact**: `IMPLEMENTATION_PLAN.md`, Step 7 and Slice B verification.

`IMPLEMENTATION_PLAN.md` instructs running
`pnpm --filter @vibe-kanban/web-core test`. The package's real name is
**`@vibe/web-core`** (`packages/web-core/package.json`). The stated command
fails with "no projects matched the filter", which reads as a broken test setup
rather than a typo.

`tasks.md` T014 avoids this by naming the target generically ("Frontend tests for
`web-core`"), so the two documents also disagree in specificity.

**Correct command**: `pnpm --filter @vibe/web-core test` (that package's `test`
script is `vitest run`).

**Status**: corrected in `IMPLEMENTATION_PLAN.md`; T014 updated to name the exact
command.

---

## Warnings

### W1 — FR-12 has no automated coverage, and its "Verified" tag overstates what was checked

**Artifacts**: `spec.md` (FR-12, acceptance criteria), `tasks.md`.

FR-12 requires that a missing prerequisite surfaces through **Vibe Kanban's**
connection test carrying the tool's own explanation. What was actually verified
(T020) is one layer down: the *server* exits and writes the missing path to
stderr. That VK's `mcp_test.rs` probe captures that stderr and surfaces it
through the assignment-test result is inferred from the existing diagnostic
path — reasonable, since `mcp_test.rs` drains stderr and attaches it to errors,
but not observed for this entry.

The acceptance criterion is tagged *"(Verified: the server exits before
completing its handshake and reports the missing file by name.)"*, which is true
of the server and does not establish the VK-surface claim the criterion makes.

**Response**: no code change. The criterion's parenthetical should be read as
server-level evidence only, and T021 (running app) is where the VK surface is
observed. If T021 cannot run in this environment, FR-12 remains **unverified**
and must be reported as such — not inferred from W1's reasoning.

### W2 — FR-10's cross-instance non-collision is only manually covered

**Artifacts**: `spec.md` (FR-10, acceptance criteria), `tasks.md` T008/T021.

T020 verified that *one* instance's tools all carry *its* prefix. The acceptance
criterion "with the disambiguator set differently on two instances, the two tool
sets do not collide" is a statement about two servers in one agent, which no
automated test reaches — the collision happens inside the *agent's* MCP client,
which VK does not host. It is genuinely only observable in T021/T022.

**Response**: accepted as a manual-only criterion, which is honest rather than a
gap to close with a test that could not actually exercise it. Report it as
unverified if T021 does not run.

### W3 — No component test for the tile behaviour change

**Artifacts**: `plan.md` Slice B, `tasks.md` T007/T008.

Constitution II asks for "a rendered-DOM component test where one already exists
for that surface". No component test exists for `McpSettingsSection.tsx` — the
MCP tests are all pure-lib (`sharedMcpSettingsState.test.ts`,
`mcpServerCodec.test.ts`, `mcpCheckSummary.test.ts`, `mcpDebugIssue.test.ts`), so
the principle's condition is not triggered and T008 satisfies it at the logic
tier.

The residual exposure is real though: T007 (dropping `disabled={added}`) is the
one change with **no automated coverage at all**. A regression that restores
`disabled` would pass every test and silently re-break multi-instance.

**Response**: accepted, given no component-test harness exists for this surface
and building one is disproportionate. T021 is the only check. Flagging it so the
gap is a decision rather than an oversight.

### W4 — FR-15 states a coupling that nothing enforces

**Artifacts**: `spec.md` FR-15, `tasks.md` T011.

"The revision pin, and every document that names it, are changed together or not
at all" is enforced only by the `AGENTS.md` note. The `GMAIL_MCP_FORK_REVISION`
constant is asserted equal to the JSON (T005), so those two cannot drift — but
the **docs page** naming the revision can drift from both silently.

This is exactly the Slack entry's situation, where the same coupling is also
documentation-only, so the plan is consistent with precedent rather than weaker
than it.

**Response**: accepted, matching precedent. Closing it would mean asserting a
docs string from a Rust test, which the repository does not do anywhere today.

---

## Info

### I1 — FR-8 (rename) is delegated to existing behaviour with no task

Correct and deliberate: rename already works
(`McpSettingsSection.tsx:568-600`). Noted only so its absence from `tasks.md`
reads as reuse (Constitution VI) rather than an omission. T021 exercises it.

### I2 — FR-9's per-instance credentials rely on untouched backend behaviour

Two instances carrying different `env` values is existing reconciliation
behaviour, not new work. Confirmed indirectly by the existing Slack conflict test
covering differing token values under one name — which is the mechanism that
makes distinct *names* mandatory (`research.md` R5).

### I3 — Both frontends are in the blast radius

`McpSettingsSection` is reached through `settingsRegistry.tsx` in `web-core`,
which serves both `local-web` and `remote-web` (Constitution IV). The change is
behaviour-preserving for every other template — tiles still add a server; they
simply no longer refuse a second one.

### I4 — Root `SPEC.md` and `specs/vk/…/spec.md` overlap by design

The pipeline produces both. They agree on every material claim (install spec,
28 tools, no icon, no audit job, neutral naming). Root `SPEC.md` carries the
technical design; `specs/vk/…/spec.md` is the WHAT/WHY per the SpecKit template.
No contradiction found; noted because two spec files invite drift if edited
separately later.

### I5 — Constitution provisions XVI and XXII were amended by this task

Both were written in stage 4, *before* `plan.md`'s Constitution Check, so the
check is against the amended text rather than retroactively justifying it. Called
out explicitly because "the plan passes a constitution the same task rewrote" is
a fair reviewer objection — the ordering is what makes it legitimate, and the
amendments generalise beyond this feature (any content-addressed pin; any
multi-instance catalog template).

### I6 — `generate-types:check` is a negative assertion (T018)

No Rust type changes, so the expectation is **no diff**. Recorded because a diff
here would indicate an unintended type change, and the reflex — committing the
regenerated file — would be wrong.

---

## Coverage matrix

| Requirement | Tasks | Automated | Manual |
| --- | --- | --- | --- |
| FR-1 catalog offers Gmail | T002, T003 | T005 | T021 |
| FR-2 immutable pin, repo matches metadata | T002, T003 | T005 | T019 ✅ |
| FR-3 placeholders, no secrets | T002 | T005 | — |
| FR-4 no shared value demanded | T002 | T005 | — |
| FR-5 works on all stdio agents | T002 | T005 | — |
| FR-6 add template twice | T006, T007 | T008 | T021 |
| FR-7 generated name accepted | T004 | T008 (property) | T021 |
| FR-8 rename | — (reuse) | — | T021 |
| FR-9 per-instance credentials | T002 | — | T021 |
| FR-10 distinct tool addressing | T002 | partial (T020 ✅, one instance) | T021, T022 |
| FR-11 documented prerequisites | T009 | — | — |
| FR-12 failure names what's missing | T009 | — | T021 (**W1**) |
| FR-13 documented prefix collision | T009 | — | — |
| FR-14 provenance check | T005 | T005 | — |
| FR-15 pin and docs move together | T011 | partial (**W4**) | — |

Every functional requirement has at least one task. Three (FR-9, FR-12, FR-10's
cross-instance half) are manual-only and will be **reported as unverified** if
T021/T022 cannot run in this environment.
