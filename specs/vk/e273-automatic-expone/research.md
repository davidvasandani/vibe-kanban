# Research: automatic capacity backoff

## Where the error comes from and how the turn is recorded
`crates/executors/src/executors/codex/normalize_logs.rs` renders
`ServerNotification::Error` as an `ErrorMessage` entry `Error: {message}`,
which matches the reported text exactly. Codex then sends `turn/completed`;
`codex/jsonrpc.rs` ends its reader loop with `ExecutorExitResult::Success`.
**Decision:** trigger on the trailing error entry for Completed and Failed
turns. **Rejected:** `status == Failed`, which never fires for Codex.

## Retry vehicle
**Decision:** a `wakeups` row delivered by the issue-comment dispatcher.
It already has session admission, busy waiting, queue priority, restart
recovery, launch evidence, archive cancellation and a UI row with Cancel.
**Rejected:**
- A shell script or poller in the workspace: not durable, no admission, and
  it would spend the agent's poller budget.
- An in-memory `tokio::time::sleep` task: lost on restart.
- A new table and sweeper: duplicates the ledger (III/VI).
- Retrying inside the Codex executor: the app-server process is torn down at
  turn end, and a long in-process wait would hold a "running" turn that the
  user cannot distinguish from work.

## Budget and archive behavior
`Wakeup::insert` enforces 8 pending / 32 lifetime and unarchives the
workspace. **Decision:** a separate `insert_capacity_retry` with no budget
check and no archive write; the agent counts exclude capacity rows. The retry
bound is the attempt count plus one row per failed turn.

## Attempt tracking
**Decision:** store `capacity_attempt` on the row; the next attempt is one more
than the attempt of the retry whose delivery's `execution_process_id` is the
failed turn. **Rejected:** parsing the attempt from the prompt (user-spoofable)
and an in-memory map (lost on restart).

## Supersession
**Decision:** a retry is valid only while the session's latest coding-agent
turn is `capacity_retry_of`, or a turn carrying the retry's own delivery marker
(a failed launch of this same retry). Checked under admission in the
dispatcher. This also makes it safe to exempt the retry from the "never resume
a stopped turn" rule: the stopped turn is the capacity failure itself, never a
user stop. **Rejected:** setting `allow_stopped_target`, which would also
re-send after an uncertain worker launch.

## Schedule
60 s doubling to 3600 s over 10 attempts is about 5 h 17 min before jitter.
Jitter of 0–20 % spreads workspaces that failed together.

## Default
Enabled by default; see `clarifications.md`.
