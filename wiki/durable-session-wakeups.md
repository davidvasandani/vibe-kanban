# Durable session wake-ups

An agent can schedule one future turn with `schedule_wakeup`: exactly one of
RFC 3339 `fire_at` (with timezone) or positive `delay_secs`, plus
`on_fire_prompt`. The deadline must be in the future and within seven days.
`list_wakeups` and `cancel_wakeup` expose the durable record. The orchestrator
can only target its own session. Global tooling must identify the target.

## Delivery admission and restart

The in-memory single-slot queued-message service cannot preserve multiple
wake-ups across restart. Store local wake-up metadata alongside a delivery
ledger row, reusing the existing durable comment dispatcher for session locks,
active-turn exclusion, user-queue priority, normal follow-up execution and
launch evidence. Local records use a nil issue ID and bypass remote comment
fetches: a local timer must not depend on the remote issue service.

Due records are swept every five seconds. A busy session receives the prompt
at an eligible turn boundary. Warm-process reuse follows the normal follow-up
path. Past-due records survive restart and include lateness in their prompt;
the resumed agent should recheck time-sensitive conditions before acting.
Missing sessions and archived, deleted or removed workspaces cancel pending
records with a stored reason. Cancellation and dispatcher admission compete
on the same pending-state transition, so a successful cancellation cannot
subsequently launch. An already claimed delivery cannot be cancelled.

Limits are eight pending records and 32 total wake-up records per session.
The total includes completed and cancelled records, bounding unattended
self-chaining even when each new timer stays below the pending limit.

## Standalone startup must use the shared initializer

In `vk/d5c5-scheduled-wakeup`, production timers stayed pending for days with
`attempt_count = 0` and no reason or error. The standalone `main.rs` duplicated
deployment initialization and omitted the dispatcher subscription, periodic
sweep and startup reconciliation marker. `startup::initialize_deployment`
already started them, but only the desktop entrypoint used it. Restarting the
standalone server could not repair missing startup wiring.

The standalone now delegates deployment initialization to that shared function.
Keep event subscription before orphan cleanup/resume and mark reconciliation
after the resume decision. Do not copy background startup calls into another
entrypoint. The source-wiring integration test in
`crates/server/tests/startup_wiring.rs` covers the production entrypoint; existing
behavioral dispatcher tests alone did not detect this omission.

For diagnosis, compare the running release's `release.json.sha` with the source,
then inspect delivery state, attempt count and reasons. A pending row with zero
attempts is not proof of an active-agent delay. Do not manually replay an overdue
operational prompt to test the scheduler. After a fixed release starts, retained
rows are reconsidered under normal busy, stopped-target and cancellation rules;
merge alone does not prove delivery.

## Condition pollers and completion receipts

`spawn_poller(on_stop_prompt=...)` additionally requires `stop_command`.
The command loop atomically publishes a receipt only when that predicate exits
zero. It contains completion time, the last tick exit status, and bounded tick
stdout/stderr. Immediate predicate success records that no tick ran. Timeout
or manual stopping never writes a successful receipt.

A receipt is the completion authority, not the original execution ID.
Workspace recovery can replay the serialized action under a new process ID,
and worker reconciliation can temporarily mark a process Indeterminate.
Neither state proves that the predicate failed or will never complete. Wait
for the receipt until the seven-day notification deadline, target invalidation,
or explicit cancellation. The operator should cancel an unwanted notification
even if its shell poller has stopped.

The receipt path is an internal field and never returned to the UI. Read the
opened descriptor with no-follow/nonblocking flags, verify it is a regular
file, and bound both bytes and read time. Workspace files can otherwise be
replaced by a FIFO or symlink and block the shared dispatcher. Validate the
receipt timestamp against observation time and the original expiry.

## Launch evidence must remain server-owned

Stored prompts and captured command output are untrusted with respect to
internal delivery markers. Quote every payload line and escape reserved marker
openers before appending the server-generated delivery marker. Quoting alone
is insufficient: a literal backslash-n in serialized JSON can resemble a line
boundary to recovery scanning. Do not let a timer payload manufacture another
record's launch evidence.

## Operator visibility

The Processes drawer shares one workspace wake-up query between its header and
list. Rows show the deadline/countdown, delivered prompt, state, cancellation
control and recorded errors. Condition notifications identify that they are
waiting for a completion receipt. A terminal cancelled record is not displayed
as a successful trigger.

## Contributed by

- vk/5ae4-pollers-add-a-ti
- vk/d5c5-scheduled-wakeup
