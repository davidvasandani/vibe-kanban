# Automatic exponential backoff for model-capacity errors

## Problem
A coding-agent turn can end with the provider error
`Error: Selected model is at capacity. Please try a different model.`
The work is unfinished, but the session simply stops and waits for a human to
type "continue". Capacity outages last minutes to hours and often hit several
workspaces at once, so unattended pipelines stall.

## Evidence from the current code
- The text is Codex's. `codex/normalize_logs.rs` renders a
  `ServerNotification::Error` as an `ErrorMessage` entry `Error: {message}`.
- After the error Codex sends `turn/completed`, the reader loop ends and
  signals `ExecutorExitResult::Success`. The turn is therefore recorded as
  **Completed**, not Failed. A trigger keyed on `status == Failed` would never
  fire; the trigger must read the turn's normalized error entries.
- `ContainerService::finalize_task` is where every exit path converges. It
  emits `TurnFinalizedEvent` for Completed and Failed turns only (never for
  Killed, Interrupted or Indeterminate), and only when no queued follow-up
  took the turn's place.
- Durable wake-ups (`wakeups` + `issue_comment_deliveries`) already deliver a
  prompt to a session at a deadline through the shared dispatcher: session
  admission, busy-turn waiting, restart survival, archive cancellation, an
  operator-visible row with a cancel control.

## Required behavior
1. **Detect.** When a coding-agent turn is finalized as Completed or Failed and
   its normalized log ends with a capacity error, the session is scheduled for
   an automatic resume. "Ends with" means an `ErrorMessage` entry matching the
   capacity pattern with no assistant message or tool use after it. Matching
   is case-insensitive on `selected model is at capacity`; the pattern list is
   one constant so further provider phrasings can be added. Text in user,
   assistant or tool entries never matches.
2. **Back off exponentially.** Attempt `n` (1-based) waits
   `min(base × 2^(n−1), max)` plus up to 20 % random jitter. Defaults: base
   60 s, max 3600 s, 10 attempts (roughly five hours in total).
3. **Chain.** A capacity failure of a turn that was itself started by capacity
   retry `n` schedules attempt `n+1`. Any other capacity failure starts at 1.
   After the last attempt nothing more is scheduled and the give-up is logged.
4. **Resume.** At the deadline the session receives a system-authored
   follow-up, with the same executor configuration as its latest turn, stating
   that the previous turn hit a capacity error and asking it to continue.
5. **Never override a human.** A pending retry is cancelled, not delivered,
   when the session's latest coding-agent turn is no longer the turn that hit
   capacity (the user or another delivery continued it), when a message is
   queued ahead of it and runs, or when the workspace is archived, deleted or
   has lost its worktree. This check runs under session admission at claim
   time. Stops, restarts and indeterminate exits never schedule a retry.
6. **Idempotent and bounded.** At most one retry row per failed turn (unique
   in the database), so duplicate events and restarts cannot double-schedule.
   Capacity retries do not consume the agent's own wake-up budget (8 pending,
   32 lifetime) and are not blocked by it. Scheduling a retry never unarchives
   a workspace.
7. **Durable.** Scheduled retries survive a server restart and fire late
   rather than never, like other wake-ups.
8. **Visible and cancellable.** The retry appears in the Processes drawer with
   its countdown and attempt number, and the operator can cancel it.
9. **Configurable.** `Config.capacity_backoff { enabled, base_delay_secs,
   max_delay_secs, max_attempts }`, read when the event is handled. Enabled by
   default: the task asks for automatic behavior, it only continues a session
   a human already started, and it is bounded per chain. A toggle in Settings
   → General turns it off.
10. **No remediation noise.** Auto error remediation does not file an
    `Auto-fix:` issue for a turn whose failure is a capacity error.

## Out of scope
- Switching to a different model. The retry keeps the session's model.
- Other transient errors (rate limits, 529 overloaded). The pattern list makes
  them a one-line addition once their exact text and semantics are confirmed.
- Changes to other services. No homelab change is needed.

## Interpretation note
The request says "run an exponential backoff script". This is implemented as a
server-owned durable schedule rather than a shell script in the workspace: a
script dies with its process and cannot be admitted safely against other
turns, while the wake-up ledger already provides both.

## Validation
- Unit tests: pattern and trailing-error detection, delay schedule and cap,
  jitter bounds, attempt chaining and exhaustion, config defaults and
  back-compatible deserialization.
- Database tests: unique-per-turn insert, budget independence in both
  directions, no unarchive, refusal for archived or deleted targets.
- Dispatcher tests: a retry resumes a Completed and a Failed capacity turn;
  a newer turn cancels it; an agent wake-up still refuses a Failed turn.
- `pnpm run check`, `pnpm run lint`, `cargo test --workspace`, generated types
  in sync, independent Codex review.
- Not claimed: live verification against a real provider outage.
