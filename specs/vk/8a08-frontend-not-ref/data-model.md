# Data Model

## FollowUpProcessBridge

Ephemeral state owned by one `ExecutionProcessesProvider`.

| Field | Type | Meaning |
| --- | --- | --- |
| key | execution process UUID | Stable deduplication identity. |
| value | `ExecutionProcess` | Complete process returned by a successful follow-up request. |
| scope | current session UUID | Provider identity boundary; not separately persisted. |

## State transitions

- Successful follow-up for active session: insert or replace by process ID.
- Duplicate successful response: no duplicate; replace the same key.
- Stream reports matching ID: stream value wins in the projection, then remove
  the bridge entry.
- Stream already contains ID before response: ignore or immediately retire the
  redundant bridge value; stream remains visible once.
- Session changes or becomes absent: clear all bridge entries.
- Mismatched response session: ignore.
- Failed follow-up: no transition.

The state is not persisted. The backend execution row and the session process
stream remain durable/authoritative.
