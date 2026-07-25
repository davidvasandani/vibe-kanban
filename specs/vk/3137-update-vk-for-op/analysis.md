# Analysis: Add Claude Opus 5 to Executor Model Selectors

**Spec**: `specs/vk/3137-update-vk-for-op/spec.md`
**Plan**: `specs/vk/3137-update-vk-for-op/plan.md`
**Tasks**: `specs/vk/3137-update-vk-for-op/tasks.md`
**Constitution**: `.specify/memory/constitution.md` (v0.10.0)
**Analyzed**: 2026-07-25

---

## 1. Source Code Verification

All claims in the spec and plan were verified against the current source.

### 1.1 `claude.rs` — Verified

| Claim | Actual | Status |
|-------|--------|--------|
| Model catalog at lines ~281-288 | Lines 282-287: `("opus", "Opus")`, `("opus[1m]", "Opus (1M context)")`, `("claude-sonnet-5", "Sonnet 5")`, `("sonnet", "Sonnet")`, `("fable", "Fable")`, `("haiku", "Haiku")` | Matches |
| `supports_effort` at line ~275-276 checks `id.contains("opus")` | Line 275-276: `\|id: &str\| -> bool { id.contains("opus") \|\| id.contains("sonnet") \|\| id.contains("fable") }` | Matches |
| `"claude-opus-5".contains("opus")` is true | Verified — automatic reasoning-option coverage | Correct |
| `mod tests` at line ~2861 | Line 2861 | Matches |

### 1.2 `cursor.rs` — Verified

| Claim | Actual | Status |
|-------|--------|--------|
| `#[schemars(description)]` at line ~50 starts with `"auto, opus-4.8, ..."` | Lines 49-50: `"auto, opus-4.8, opus-4.6, sonnet-4.6, gpt-5.4, ..."` | Matches |
| Claude arms in `resolve_cursor_model_name` at lines ~99-108 | Lines 99-108: `opus-4.8`, `opus-4.6`, `sonnet-4.6`, `opus-4.5`, `sonnet-4.5` with standard/thinking variants | Matches |
| Claude reasoning arm at line ~125 | Line 125: `"opus-4.8" \| "opus-4.6" \| "sonnet-4.6" \| "opus-4.5" \| "sonnet-4.5"` | Matches |
| Catalog entry `("opus-4.8", "Claude 4.8 Opus")` at line ~655 | Line 656: `("opus-4.8", "Claude 4.8 Opus")` | Matches (off by 1) |
| `mod tests` at line ~1404 | Line 1404 | Matches |

### 1.3 `copilot.rs` — Verified

| Claim | Actual | Status |
|-------|--------|--------|
| `("claude-opus-4.8", "Claude Opus 4.8")` at line ~201 | Line 201 | Matches |
| No `#[schemars(description)]` on `Copilot.model` | Confirmed — `model` field has no description annotation | Matches |
| No `mod tests` block | Confirmed | Matches |
| Struct fields: `append_prompt`, `model`, `allow_all_tools`, `allow_tool`, `deny_tool`, `add_dir`, `disable_mcp_server`, `cmd`, `approvals` | Lines 27-48 — all 9 fields confirmed | Matches |

### 1.4 `droid.rs` — Verified

| Claim | Actual | Status |
|-------|--------|--------|
| `#[schemars(description)]` at line ~72 with model examples | Line 72: `"Model to use (e.g., gpt-5-codex, claude-sonnet-4-5-20250929, ...)"` | Matches |
| `("claude-opus-4-8", "Claude Opus 4.8")` at line ~241 | Line 241 | Matches |
| No `mod tests` block | Confirmed | Matches |
| Struct fields: `append_prompt`, `autonomy`, `model`, `reasoning_effort`, `cmd` | Lines 58-85 — all 5 fields confirmed | Matches |
| `Autonomy::Normal` variant exists | Line 32 | Confirmed |

### 1.5 Supporting Types — Verified

| Type | Status |
|------|--------|
| `CmdOverrides` derives `Default` | Line 43 of `command.rs` — `#[derive(..., Default)]` |
| `AppendPrompt` implements `Default` | Used via `AppendPrompt::default()` across existing tests |

---

## 2. Constitution Compliance

### Applicable Principles

| Principle | Verdict | Notes |
|-----------|---------|-------|
| **I. Clarity over cleverness** | Pass | All changes follow existing naming conventions and catalog patterns verbatim. No non-obvious choices. |
| **II. Test the contract** | Pass | Acceptance criteria defined in spec (Testing section). Unit tests specified for all four executors: catalog presence, reasoning resolution (Cursor), reasoning options coverage (Claude Code `supports_effort`, Cursor). |
| **III. Small, reversible steps** | Pass | One model entry per executor. Purely additive — no reordering, no removals. Reuses existing `supports_effort` closure and `cursor_reasoning_options` patterns rather than duplicating logic. |
| **VI. Don't rebuild what shipped** | Pass | Extends existing model catalog arrays and match arms. No new code paths, abstractions, or parallel machinery. |
| **IX. External agent protocols** | Pass | Preserves stable serialized executor identifiers (struct shapes unchanged). Only runtime `discover_options()` values change. Unknown-event degradation unaffected. |

### Non-Applicable Principles (confirmed N/A)

IV (shared-component boundaries — no frontend changes), V (remote mutations),
VII (workspace breadcrumbs), VIII (managed tools), X (dialog state),
XI (diagnostics), XII (async handoffs), XIII (vendor config files).

### Constraints Checklist

| Constraint | Verdict | Notes |
|------------|---------|-------|
| Follow existing architecture/conventions | Pass | Provider-specific naming conventions followed per executor. |
| No new top-level dependencies | Pass | No dependency additions. |
| Generated files never edited by hand | Pass | T009 regenerates via `pnpm run generate-types`. |
| Executor additions: contract regen, mapping checks, fixture tests | See below | |
| Run `pnpm run format` before completing | Pass | T012. |

**On "Executor additions" constraint:** This constraint targets adding *new executors*
(cf. the `grok-executor-integration` knowledge-base entry it was codified from).
This task adds model-catalog entries to *existing* executors with no struct,
serialization, or protocol changes. Nonetheless, the plan satisfies the
spirit of the constraint:

- **Generated-contract regeneration:** T009 regenerates all schemas; T011 verifies
  sync. The plan correctly identifies that only `cursor_agent.json` and `droid.json`
  will change (description annotations); `claude_code.json` and `copilot.json`
  have no model-listing annotations and will not change.
- **Backend/frontend mapping:** The UI consumes `discover_options()` generically
  via `ModelSelectorConfig`. No frontend code enumerates model IDs, so no mapping
  check is needed. This reasoning is sound and documented in plan section 7.
- **Fixture-based protocol tests:** The event protocol is unchanged (no new fields,
  no new serialization variants). Unit tests verifying catalog contents and
  reasoning resolution are proportionate. Fixture-based tests would add no
  value here.

---

## 3. Cross-Document Consistency

### 3.1 Spec vs. Plan

| Area | Consistent? | Notes |
|------|-------------|-------|
| Affected executors (4) | Yes | Both list Claude Code, Cursor, Copilot, Droid. |
| Unaffected executors (7) | Yes | Both exclude Amp, Codex, Gemini, Grok, OpenCode, Qwen (spec also names Amp explicitly in plan context). |
| Claude Code: entry `("claude-opus-5", "Opus 5")` | Yes | Identical in spec section 1 and plan section 2.1. |
| Claude Code: placement after `opus[1m]` | Yes | |
| Cursor: 4 coordinated edits | Yes | Description, resolution, reasoning options, catalog. |
| Cursor: display label `"Claude 5 Opus"` | Yes | Follows `"Claude {version} {family}"` pattern confirmed in source. |
| Copilot: entry `("claude-opus-5", "Claude Opus 5")` | Yes | |
| Droid: entry `("claude-opus-5", "Claude Opus 5")` | Yes | |
| Droid: description string update | Yes | |
| No fast-mode variants | Yes | Spec non-goal C4 matches plan scope exclusion. |
| No default model changes | Yes | Both explicitly exclude. |
| Schema regeneration strategy | Yes | Both identify `cursor_agent.json` and `droid.json` as changing. |

### 3.2 Plan vs. Tasks

| Area | Consistent? | Notes |
|------|-------------|-------|
| T001 maps to plan 2.1 | Yes | Claude Code catalog insertion. |
| T002 maps to plan 2.2 (a-d) | Yes | All four Cursor edits captured. |
| T003 maps to plan 2.3 | Yes | Copilot catalog insertion. |
| T004 maps to plan 2.4 (a-b) | Yes | Droid description + catalog. |
| T005-T008 map to plan 4.1 | Yes | Test per executor. |
| T009 maps to plan section 3 | Yes | Regeneration step. |
| T010-T013 map to plan 4.2 | Yes | Verification commands. |
| Layer dependencies | Yes | Correctly models parallelism and sequential constraints. |

### 3.3 Spec vs. SPEC.md (Root)

| Area | Consistent? | Notes |
|------|-------------|-------|
| Requirement 1: Claude Code explicit entry | Yes | Both agree after clarification C1/C5. |
| Requirement 5: generated artifacts via documented commands | Yes | |
| Verification: independent Codex review | Yes | SPEC.md and tasks T014 both require it. |

---

## 4. Findings

### F1. No blocking issues found

All source claims verified. All constitution principles satisfied. All three
documents (spec, plan, tasks) are internally consistent and mutually aligned.

### F2. Test code in plan is compilable (verified)

- `CursorAgent` struct: 5 fields — all initialized in plan test code. `CmdOverrides`
  derives `Default`. Confirmed.
- `Copilot` struct: 9 fields including `approvals: Option<Arc<dyn ExecutorApprovalService>>`
  with `#[serde(skip)]` — correctly initialized as `None` in plan test. Confirmed.
- `Droid` struct: 5 fields — `Autonomy::Normal` variant exists (line 32). Confirmed.

### F3. Minor line-number drift (no action needed)

Plan uses `~` prefix on all line numbers. Maximum observed drift is 1 line
(Cursor catalog: plan says ~655, actual is 656). All references land within
the correct function and array context.

### F4. Task T015 knowledge-base article is well-motivated

PRIOR_KNOWLEDGE.md constraint 4 explicitly calls for recording a reusable
procedure if implementation reveals one. T015 addresses this.

### F5. Task T014 Codex review is spec-required

SPEC.md Verification section requires "an independent Codex diff review."
T014 captures this. It correctly depends on all verification steps (T013).

---

## 5. Recommendations

None. The spec, plan, and tasks are ready for implementation as written.

---

## 6. Verdict

**PASS** — No gaps, contradictions, missing coverage, or constitution violations
found. Implementation may proceed using the task list in `tasks.md`.
