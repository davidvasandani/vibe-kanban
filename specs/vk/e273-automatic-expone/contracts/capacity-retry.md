# Contracts: automatic capacity backoff

## Policy functions (`services::capacity_backoff`)
- `is_capacity_error(text) -> bool`: case-insensitive substring match against
  `CAPACITY_ERROR_PATTERNS`.
- `turn_ended_with_capacity_error(entries) -> bool`: scanning from the end,
  true at the first matching `ErrorMessage`; false at the first
  `AssistantMessage` or `ToolUse`; other entry types are skipped; false when
  exhausted.
- `backoff_delay(attempt, base, max) -> Duration`:
  `min(base × 2^(attempt−1), max)`, saturating; attempt 0 is treated as 1.
- `with_jitter(delay, fraction) -> Duration`: `delay × (1 + 0.2 × fraction)`
  with `fraction` clamped to `[0, 1]`.
- `next_attempt(previous, max_attempts) -> Option<u32>`: `previous + 1` or 1,
  `None` when it would exceed `max_attempts`.
- `retry_message(attempt, max_attempts) -> String`.

## Storage (`db::models::wakeup::Wakeup`)
- `insert_capacity_retry(&self, pool) -> Result<bool, sqlx::Error>`: true when
  a row was created. False when a retry for that execution already exists or
  the target is archived, deleted, without a worktree, or the session is not
  the workspace's.
- `capacity_attempt_for_execution(pool, execution_id) -> Result<Option<i64>>`.

## Wire types
`Wakeup` gains `capacity_retry_of: string | null` and
`capacity_attempt: number | null`. `Config` gains `capacity_backoff`.
No new HTTP route and no MCP tool change. `list_wakeups` and `cancel_wakeup`
show and cancel retries like any wake-up.

## Delivered prompt
The dispatcher quotes the stored message and appends its delivery marker, as
for every local wake-up. The message names the attempt, states that the
previous turn ended with a provider capacity error rather than completion, and
asks the agent to review the working tree and continue.
