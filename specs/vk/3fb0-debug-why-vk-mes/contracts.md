# Contracts: vk/3fb0-debug-why-vk-mes

## Wire contract: unchanged

`GET /api/execution-processes/{id}/messages?limit&all&roles`
→ `ApiResponse<RecentMessagesResponse>`

```
RecentMessagesResponse {
  session_id: string
  execution_id: string
  status: ExecutionProcessStatus
  exit_code: number | null
  final_message: string | null
  messages: SessionMessage[]      // { id, role, text, created_at, execution_id }
  has_more: boolean
}
```

No field is added, removed, retyped, or given a new meaning. MCP tools
`list_recent_messages` (clamped tail, default 20, max 100) and
`list_all_messages` (`all=true`) keep their current inputs and outputs.

## Behavioural contract

| Execution state | Today | After |
| --- | --- | --- |
| Running, live store | **Never returns**; caller times out | Returns buffered messages so far, promptly |
| Running, no messages yet | **Never returns** | Returns `messages: []`, promptly |
| Finished, cached sidecar | Returns from cache | Unchanged |
| Finished, cache miss | Single-flight historical re-normalization | Unchanged |
| Unknown execution id | 404 via route middleware | Unchanged |

Invariants that must hold identically in both columns:

- chronological order and `{execution_id}:{index}` identity;
- role filtering (`user`, `assistant`, `system`, `tool`) and the entry-type
  mapping in `entry_role`, including `Loading` / `TokenUsageInfo` exclusion;
- 4000-character per-message truncation with the `… [truncated]` marker;
- `has_more` meaning "more matched than `limit` allowed through", always
  `false` for `all=true`;
- repo-diff patches excluded from messages;
- no side effects — the read writes no sidecar and migrates no affinity.

## Internal interfaces added

Free functions in `crates/services/src/services/container.rs`, private to the
crate, DB-free so they are directly testable:

```rust
fn indexed_entry_patches_from_history(msg_store: &MsgStore) -> Vec<Patch>;
fn entries_from_patches(id: &Uuid, patches: &[Patch]) -> Option<Vec<NormalizedEntry>>;
fn normalized_entries_from_history(id: &Uuid, msg_store: &MsgStore) -> Option<Vec<NormalizedEntry>>;

// The source choice itself, so the live-store preference is testable
// without a database or a ContainerService.
async fn normalized_entries_from_sources<Fut>(
    id: &Uuid,
    live_store: Option<Arc<MsgStore>>,
    settled_stream: impl FnOnce() -> Fut,
) -> Option<Vec<NormalizedEntry>>;
```

Also added, outside this file:

```rust
// crates/utils/src/msg_store.rs — clone only the selected variant, under the
// read guard, instead of deep-copying all retained history.
pub fn select_history<T>(&self, select: impl FnMut(&LogMsg) -> Option<T>) -> Vec<T>;

// crates/services/src/services/normalized_log_cache.rs — skip non-applying
// patches instead of abandoning the document; live-store path only.
pub fn materialize_entries_lossy(patches: &[Patch]) -> Result<(Vec<Value>, usize), CacheError>;
```

`ContainerService::normalized_entries` keeps its signature
(`async fn(&self, &Uuid) -> Option<Vec<NormalizedEntry>>`); only its source
selection changes. `stream_normalized_logs` is not modified.
