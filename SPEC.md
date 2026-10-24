# Durable poller wake-ups

Task: vk/5ae4-pollers-add-a-ti

## Outcome
Agents can schedule a single follow-up on their creating session, end their turn,
and resume when a persisted deadline or poller completion condition arrives.
Operators can inspect and cancel pending wake-ups in Processes.

## Contract
- Support a timezone-aware RFC 3339 fire_at or positive delay_secs, exclusively,
  with a nonblank on_fire_prompt; timers need no shell command or interval.
- Existing interval pollers retain their completion/lifetime requirements and
  may supply on_stop_prompt, delivered only on successful completion predicate.
- Persist session identity, deadline, prompt, trigger context and delivery state.
  Restart recovery delivers overdue timers with explicit lateness and a reminder
  to recheck time-sensitive conditions. Include bounded last output/exit status.
- Use the existing follow-up/resume lifecycle, queue behind active turns, preserve
  operator messages, and respect warm-process and execution admission gates.
- Cancellation is durable and serialized with delivery. Cancelled pending work
  cannot initiate a turn. Record invalid-target cancellation (missing session,
  deleted or archived workspace) without cascading away the audit record.
- Bound scheduling to seven days, eight pending wake-ups per session and a
  persistent automatic continuation budget of 32 per session to cap self-chaining.
- UI displays deadline, live countdown, stored prompt, state/reason and Cancel.

## Design constraints
The current QueuedMessageService is an in-memory single-message slot that
replaces previous messages. It cannot itself be the durable timer source of
truth. Reuse existing durable continuation/admission machinery where possible;
retain wake-ups pending until delivery can safely be claimed. Define crash
boundaries explicitly and test recovery without blindly duplicating execution.
Only the Vibe Kanban service is in scope; no other service updates.

## Acceptance and verification
Regression tests cover scheduled firing, stop-predicate delivery, cancellation,
restart and late firing, busy-session queueing, missing/archived/deleted targets,
limits and queue preservation. Verify the rendered timer list and cancellation.
Run repository formatting/checks and independent Codex review, address findings,
record reusable knowledge, then open and merge a PR against the base branch.
