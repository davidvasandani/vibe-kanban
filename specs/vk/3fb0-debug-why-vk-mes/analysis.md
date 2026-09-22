# Analysis: vk/3fb0-debug-why-vk-mes

Cross-check of `spec.md`, `plan.md`, `tasks.md` against
`.specify/memory/constitution.md` v0.34.0. Findings only — spec, plan, and
tasks were not modified by this stage.

## Errors

None. No requirement is contradicted across artifacts, and no constitution
principle is violated by the planned approach.

## Warnings

- **W-1 (tasks.md, spec.md) — Acceptance criterion 1 is only half covered.**
  The criterion asserts a running read returns messages *"with its status
  reported as running"*, and FR-3 requires the response to carry authoritative
  status. But T005–T007 all exercise `normalized_entries_from_history`, which
  returns `Vec<NormalizedEntry>` and carries no status. Status is attached
  later, in `build_recent_messages_response`, from `execution_process.status`.
  The status half of the criterion is satisfied by existing untouched code and
  is not asserted anywhere in the new tests. Accept as a documented limitation
  (route-level tests need a `DeploymentImpl`, which this module's DB-free test
  style deliberately avoids) or downgrade the criterion to match what is tested.

- **W-2 (spec.md, tasks.md) — FR-9 has no coverage.** "For a running execution
  the reported final message MUST be the latest assistant text produced so far"
  is a behaviour change in effect (today a running read produces nothing at
  all), yet no task asserts it. `last_assistant_message` is already unit-tested
  over an entry slice, but not for the running case. Cheap to add to T005 by
  asserting the last assistant entry is present in the snapshot.

- **W-3 (tasks.md) — T006 risks being brittle or slow.** Asserting that a
  stream does *not* terminate can only be done by waiting. It must use a short
  deadline and assert the timeout elapsed, never a long sleep, or it will add
  dead wall-clock to every suite run. If it cannot be made both fast and
  non-flaky, T006 should be dropped: T005 under a deadline already fails on
  regression, and T006 is corroborating evidence rather than the guard itself.

- **W-4 (spec.md) — FR-6 is asserted only structurally.** "A read MUST NOT
  trigger reconstruction work for an execution whose messages are already
  available in memory" holds because the live-store branch returns before
  reaching the cache and semaphore paths. Nothing tests it, and a later edit
  could reorder the branches without failing anything. Low risk given T004
  explicitly comments the ordering; noted so it is a decision rather than an
  oversight.

## Info

- **I-1 (plan.md) — the storeless-running case was checked and is safe.** An
  execution that is `Running` but has no in-memory store (typical after a server
  restart mid-turn) falls through to the historical branch. That path builds a
  `temp_store` from a finite raw log and calls `push_finished()`
  (`container.rs:~1577`), so it terminates. It is expensive, but bounded by
  `MAX_HISTORICAL_NORMALIZATION_MSGS` and single-flighted per constitution XXXI.
  FR-6 does not forbid it, since those messages are not "already available in
  memory". No artifact change needed; recorded so the case is not re-derived.

- **I-2 (tasks.md) — pipeline stage 13 (open and merge the PR) is not a task.**
  Intentional: it is pipeline scope, not feature scope. Noted so the tasks list
  is not read as the complete definition of done.

- **I-3 (constitution) — XXXVIII was added for this task,** so the plan
  satisfying it is not independent evidence of good design. The substantive
  checks are II (the deadline-bounded regression test), III/VI (reuses
  `get_history` and an existing filter; no new read path or dependency), and
  XXXI (the completed-history machinery is untouched). All hold.

- **I-4 (spec.md) — FR-4/FR-7 are structurally guaranteed.** `project_messages`
  and `entry_role` operate on the materialized entry vector and never see the
  source, so recent/all selection, role filtering, truncation, and `has_more`
  cannot diverge between running and finished reads. Their existing unit tests
  remain valid without duplication for the running case.

## Disposition

W-2 will be folded into T005 (a one-line assertion). W-3 will be applied when
writing T006, dropping it if it cannot be made fast and deterministic. W-1 and
W-4 are accepted as documented limitations of the DB-free test boundary this
module already uses.
