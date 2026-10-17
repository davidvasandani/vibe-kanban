# Data model (in-memory only)

Nothing is persisted, and the schema does not change.

## `DiffStatsCache` slot (`crates/services/src/services/workspace_diff_stats.rs`)

| Field | Type | Meaning |
| --- | --- | --- |
| `state` | `Arc<tokio::Mutex<Option<Entry>>>` | The authoritative cached entry (outcome, `computed_at`, generation). A running leader holds it for the whole computation. Unchanged. |
| `generation` | `AtomicU64` | Bumped by `invalidate`. A result from an older generation never becomes `state`. Unchanged. |
| `last_known` | `std::sync::Mutex<Option<DiffStats>>` | **New.** The stats of the most recent computation that finished successfully, whatever its generation. Readable without `state`'s async lock. Used only by `get_within` when the deadline expires. It is never treated as fresh and is never returned by `get_or_compute`. |

Lifecycle of `last_known`:
- It is `None` when the slot is created, including after a slot is pruned.
- The leader sets it to `Some(stats)` whenever a compute returns
  `Some(outcome)`, including results that are complete, incomplete, or
  invalidated during the computation.
- It is unchanged when a compute returns `None` (a failure), and unchanged by
  `invalidate`.
