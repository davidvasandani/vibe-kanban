# Automatic capacity backoff: continuing a session after a transient provider error

When a coding-agent turn ends with `Selected model is at capacity. Please try
a different model.`, VK resumes the session by itself after an exponentially
growing delay (`Config.capacity_backoff`, on by default; constitution LX).
This page records what is reusable for any "continue the session
automatically" feature.

## A provider error does not make the turn Failed

The text is Codex's. Codex is signal-driven (see
[[agent-process-lifecycle]]): after the error it sends `turn/completed`, the
reader loop ends, and the exit signal is `Success`. The turn is recorded
**Completed**. Anything keyed on `status == Failed`, including auto error
remediation, never sees it.

The trigger is therefore evidence in the turn's own normalized log:
`turn_ended_with_capacity_error` walks the entries from the end and returns
true at an `ErrorMessage` entry matching the pattern, false at the first
`AssistantMessage` or `ToolUse`. Text in user, assistant or tool content is
never evidence. To support another provider phrasing, add a lowercase
substring to `CAPACITY_ERROR_PATTERNS` once its exact text is confirmed.

## Reading a just-finished turn's log

`finalize_task` and log normalization are separate pipelines. A fixed delay
before `normalized_entries` is not enough: with a live `MsgStore` the read is
a partial snapshot, and a miss is permanent because nothing re-inspects the
turn (Codex review, round 1). Wait, bounded, until
`get_msg_store_by_id(id)` returns `None`. The store is removed when the log
is settled, which is the same signal `normalized_log_snapshot` reports as
`complete`. Past the deadline, judge what is readable. `None` from
`normalized_entries` is "unreadable", not "no errors": schedule nothing.

Identify the turn from the database, not from the event:
`TurnFinalizedEvent.execution_process_id` can be a cleanup script. Take the
session's latest coding-agent turn and require
`SessionTurnBoundary::finalized_turn` to name it
([[issue-comment-delivery]]).

## Reuse the wake-up ledger, with its own insert

The retry is a `wakeups` row with `capacity_retry_of` (the failed execution)
and `capacity_attempt`, delivered by the shared dispatcher
([[durable-session-wakeups]]). `Wakeup::insert` is wrong for system-scheduled
rows in three ways, so `insert_capacity_retry` exists:

- It enforces the agent's 8 pending / 32 lifetime budget. Capacity rows are
  excluded from those counts in both directions.
- It unarchives the workspace. A system retry must refuse an archived
  workspace instead.
- It has no idempotence. A unique partial index on `capacity_retry_of` plus
  `INSERT OR IGNORE` makes a duplicate event a no-op.

`Wakeup::message` returns the stored prompt unwrapped for these rows. The
default wrapper calls the prompt "agent-authored", which is false here.

## Attempt chaining without extra state

The next attempt is one more than the attempt of the retry whose delivery
row's `execution_process_id` is the failed turn
(`capacity_attempt_for_execution`). A capacity failure of any other turn
starts at 1. Parsing the attempt out of the prompt would be user-spoofable,
and an in-memory counter is lost on restart.

## Never override a newer turn

A pending retry is valid only while the session's latest coding-agent turn is
`capacity_retry_of`, or a turn carrying the retry's own delivery marker (its
own launch that failed before the agent started). Otherwise
`capacity_retry_superseded` is true and the row is cancelled with a stored
reason.

- The authoritative check is in `dispatch_session_admitted`, under session
  admission, before the claim. `wakeups::reconcile` repeats it every sweep so
  the row disappears promptly. Supersession is monotonic, so the sweep needs
  no lock.
- This proof is what allows the retry to resume a **Failed** turn. Do not set
  `allow_stopped_target` for that: it also re-sends after an uncertain worker
  launch. A validated-row set local to the dispatch keeps the uncertain-launch
  rule intact.

## Gotchas

- `checked_shl` plus `checked_mul` for the delay, and `checked_add` for the
  attempt. `saturating_add` at `u32::MAX` stays within `<= max_attempts` and
  never exhausts.
- Adding fields to `Wakeup` breaks struct literals in four files, including
  tests in `db/src/models/workspace.rs`. The positional
  `INSERT INTO wakeups VALUES (...)` had to become a named-column insert.
- Tests that set `issue_comment_deliveries.execution_process_id` to an
  execution that does not exist need `PRAGMA foreign_keys = OFF`.
- `cargo test -p server` builds every binary's tests and does not finish in a
  ten-minute foreground command on the agent host. Use `--lib` and
  `--test <name>` ([[host-env-sensitive-tests]]).
- A failed capacity turn would file an `Auto-fix:` issue. The remediation
  consumer skips it before reserving a guard slot
  ([[auto-error-remediation]]).

## Not covered

- A capacity failure whose event is lost to a restart before the consumer
  handles it is not retried. There is no boot-time catch-up.
- The retry keeps the session's model. It never switches model.
- Only the Codex phrasing is matched.

## Contributed by

- `vk/e273-automatic-expone`
