# Data model: automatic capacity backoff

## `wakeups` (existing table, two new nullable columns)
| Column | Type | Meaning |
| --- | --- | --- |
| `capacity_retry_of` | BLOB (uuid), null | Execution whose capacity failure this row retries. Null for agent wake-ups and poller notifications. |
| `capacity_attempt` | INTEGER, null | 1-based position in the retry chain. Set exactly when `capacity_retry_of` is set. |

Index: `UNIQUE (capacity_retry_of) WHERE capacity_retry_of IS NOT NULL`.

Rules:
- One retry per failed execution (the unique index).
- Agent budget counts only rows with `capacity_retry_of IS NULL`.
- `prompt` holds the full system-authored message for capacity rows.
- The paired `issue_comment_deliveries` row is unchanged in shape: nil issue
  id, `next_attempt_at = fire_at`, state pending → dispatching → delivered, or
  cancelled with a reason in `error`.

## `Config.capacity_backoff`
| Field | Type | Default |
| --- | --- | --- |
| `enabled` | bool | true |
| `base_delay_secs` | u32 | 60 |
| `max_delay_secs` | u32 | 3600 |
| `max_attempts` | u32 | 10 |

A missing object or field takes its default. `max_attempts = 0` schedules
nothing. A zero base delay is treated as one second.

## State of a retry
pending → (deadline, session idle, turn unchanged) → dispatching → delivered
pending → cancelled: operator, superseded by a newer turn, workspace archived
or deleted, session deleted.
