# Implementation plan: automatic capacity backoff

Implements `SPEC.md`. Each step lists its files and its test.

## 1. Pure policy — `crates/services/src/services/capacity_backoff.rs` (new)
- `CAPACITY_ERROR_PATTERNS` (lowercase substrings) and `is_capacity_error`.
- `turn_ended_with_capacity_error(&[NormalizedEntry]) -> bool`: walk entries
  from the end; an `ErrorMessage` matching a pattern returns true; an
  `AssistantMessage` or `ToolUse` seen first returns false.
- `backoff_delay(attempt, base, max)`, saturating, capped.
- `with_jitter(delay, fraction)`, adding up to 20 %.
- `next_attempt(previous, max_attempts) -> Option<u32>`.
- `retry_message(attempt, max_attempts)`: the system-authored prompt.
- Register in `services/mod.rs`. Unit tests for every function.

## 2. Config — `crates/services/src/services/config/versions/v8.rs`
- `CapacityBackoffConfig { enabled, base_delay_secs, max_delay_secs,
  max_attempts }` with `#[serde(default)]` on the struct and on the `Config`
  field. Defaults: true, 60, 3600, 10.
- Re-export beside `AutoErrorRemediationConfig`; add to both `Config`
  constructors and to `generate_types.rs`.
- Test: a config JSON without the field deserializes to the defaults.

## 3. Storage — `crates/db`
- Migration `20261118000000_capacity_retry_wakeups.sql`: add nullable
  `capacity_retry_of BLOB` and `capacity_attempt INTEGER` to `wakeups`, plus a
  unique partial index on `capacity_retry_of`.
- `Wakeup`: two new optional fields. `insert` names its columns and counts
  only rows where `capacity_retry_of IS NULL` against the agent budget.
- `Wakeup::insert_capacity_retry`: one immediate transaction that requires an
  existing, unarchived workspace with a worktree and a matching session,
  inserts with `INSERT OR IGNORE` on the unique index, adds the delivery row
  only when the wake-up row was inserted, and never touches `archived`.
  Returns whether a row was created.
- `Wakeup::capacity_attempt_for_execution(execution_id)`: the attempt number
  of the capacity retry whose delivery started that execution.
- `Wakeup::message` returns the stored system prompt, unwrapped, for capacity
  rows.
- Tests in `wakeup.rs` on an in-memory migrated database.

## 4. Dispatcher guard — `crates/server/src/issue_comment_delivery.rs`, `wakeups.rs`
- `wakeups::capacity_retry_superseded(pool, &Wakeup)`: true when the session's
  latest coding-agent turn is neither `capacity_retry_of` nor a turn carrying
  this wake-up's own delivery marker.
- In `dispatch_session_admitted` (under session admission): cancel superseded
  capacity rows with a stored reason and collect the ids of valid ones. A valid
  capacity row is exempt from the "never resume a stopped turn" rule, because
  the stopped turn is proven to be the capacity failure. The uncertain-launch
  rule is unchanged.
- In `wakeups::reconcile`: the same cancellation, so a superseded retry
  disappears within one sweep instead of at its deadline.
- Tests in `issue_comment_delivery/tests.rs` using the existing mock backend.

## 5. Consumer — `crates/server/src/capacity_backoff.rs` (new)
- `spawn(&deployment)`: subscribe to `turn_finalized_sender`; one task per
  event. Wire it in `startup::initialize_deployment` next to the other
  subscribers and pin it in `tests/startup_wiring.rs`.
- `handle`: read config (return when disabled); load the session's latest
  coding-agent turn; require Completed or Failed and that
  `SessionTurnBoundary::finalized_turn` names it; wait a one-second settle
  delay; read `normalized_entries` (`None` schedules nothing); apply
  `turn_ended_with_capacity_error`; compute the attempt from
  `capacity_attempt_for_execution`; give up with a warning past
  `max_attempts`; otherwise `insert_capacity_retry` with the jittered deadline.
- The decision is a function over loaded inputs so it is tested without a
  deployment.

## 6. Remediation exclusion — `crates/server/src/error_remediation.rs`
- Before reserving a guard slot, skip when the failed turn ended with a
  capacity error.

## 7. Frontend
- Regenerate `shared/types.ts`.
- `WakeupRow.tsx`: title "Capacity retry (attempt N)" for capacity rows; test.
- `GeneralSettingsSection.tsx`: a checkbox under task execution bound to
  `capacity_backoff.enabled`, replacing the whole object on change. Strings in
  all seven locales.

## 8. Docs
- A short section in the docs page that covers interrupted-run resume or
  wake-ups, describing the schedule, cancellation rules and the setting.

## 9. Verify
`pnpm install --frozen-lockfile`, `pnpm run generate-types`,
`pnpm run format`, `pnpm run check`, `pnpm run lint`,
`cargo test --workspace` (host-env-sensitive tests re-run with their variables
unset if they fail), then Codex review, wiki update, PR and merge.
