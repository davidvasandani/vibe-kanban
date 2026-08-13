# Data Model: Concatenate Repeating Lines

All state is process-local inside one Codex log-normalization stream.

## `RepeatedCommand`

- `entry_index: usize` — original normalized row shared by the run.
- `command: String` — exact normalized eligible command used for equality.
- `count: usize` — total occurrences represented by the row, including the
  first.
- `latest_call_id: String` — only this lifecycle may replace the shared row.
- `latest_completed: bool` — true only after the latest occurrence succeeds;
  required before the next occurrence can reuse the row.

## `CommandState` addition

- `repeat_count: usize` — total occurrences represented by this command's
  visible row. Defaults to one and is copied into every streamed/completed
  replacement so the marker is stable.

## State transitions

- `none -> active(count=1, incomplete)` on the first eligible command.
- `active(success) -> active(count+1, incomplete)` on an adjacent identical
  eligible command with a new call ID.
- `active(incomplete) -> active(success)` on successful owner completion.
- `active(incomplete) -> active(failed)` on unsuccessful owner completion;
  the row is failed and a future call allocates a new row.
- Any changed/intervening command fails the adjacency/equality guard and starts
  a new tracker at count one.
