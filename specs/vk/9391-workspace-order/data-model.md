# Data Model: Workspace Sort Projection

## Existing Entity: `SidebarWorkspace`

Relevant fields:

- `id: string` — unique stable identity and final tie-breaker.
- `name: string` — user-facing label and first tie-breaker.
- `isPinned?: boolean` — highest-priority ordering partition.
- `createdAt: string` — base created-time value.
- `updatedAt: string` — persisted base workspace update time, available with the
  workspace stream.
- `latestProcessCompletedAt?: string` — optional richer activity time supplied
  by the asynchronous summary query.

## Derived Value: Selected Sort Timestamp

- Updated sort: first valid value of `latestProcessCompletedAt`, `updatedAt`.
- Created sort: valid `createdAt`.
- Invalid or absent strings derive to no timestamp.

## Ordering Invariants

1. Pinned partition precedes unpinned partition.
2. A valid selected timestamp precedes an absent selected timestamp.
3. Direction applies only between two valid selected timestamps.
4. Equal/absent timestamps compare by name, then unique ID.
