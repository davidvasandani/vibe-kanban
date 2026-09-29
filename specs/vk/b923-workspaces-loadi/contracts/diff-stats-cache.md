# Contract: `DiffStatsCache::get_within`

```rust
pub async fn get_within<F>(
    &self,
    workspace_id: Uuid,
    max_age: Duration,
    deadline: Duration,
    compute: F,
) -> Option<DiffStats>
where
    F: Future<Output = Option<DiffStatsOutcome>> + Send + 'static;
```

| Situation | Returns | Side effects |
| --- | --- | --- |
| A fresh entry exists (within `max_age`, current generation) | The fresh stats, even if `deadline` is zero | none |
| The compute finishes and succeeds before `deadline` | The new stats | Publishes the entry (if the generation matches) and `last_known` |
| The compute finishes and fails (`None`) before `deadline` | `None` | Nothing is cached, and `last_known` is unchanged |
| `deadline` expires first | `last_known` (possibly `None`) | The leader keeps running, then publishes the entry and `last_known` |
| Another leader is computing and `deadline` expires | `last_known` | No extra compute starts |

Invariants that stay the same: single-flight per workspace, at most
`BULK_DIFF_STATS_CONCURRENCY` computations process-wide, and invalidation wins
for the authoritative entry.

# HTTP contract

`POST /api/workspaces/summaries` keeps the same request and response shape.
New guarantee: the diff-stat phase takes at most `SUMMARY_DIFF_STATS_BUDGET`
(3 s) of the response time. A workspace's `files_changed`, `lines_added` and
`lines_removed` fields may hold its last observed values when a fresh
computation is still running.
