# Data Model: Codex Rollout Lineage Transfer

## `codex_session_transfers`

One row per affinity operation.

| Field | Meaning |
| --- | --- |
| `operation_id` PK/FK | Workspace affinity operation and retry identity |
| `workspace_id` | Authorized workspace |
| `source_execution_id` | Running Codex execution whose session is moved |
| `source_worker_node_id` | Source node; nullable sentinel policy for local |
| `target_worker_node_id` | Target node; nullable sentinel policy for local |
| `leaf_thread_id` | Requested Codex thread UUID |
| `manifest_digest` | Canonical SHA-256 of ordered manifest |
| `phase` | `claimed`, `manifested`, `staging`, `verified`, `failed` |
| `verified_at` | Target complete-lineage verification time |
| `last_needed_at` | Retry/continuation retention clock |
| `failure_category` | Allow-listed safe category |
| `failure_phase` | Safe phase name |
| `failure_detail_json` | Bounded safe identifiers/counts/checksums only |
| timestamps | Created/updated for stale-claim recovery |

Unique `operation_id` binds a transfer to one affinity operation. A check
constraint limits phases. Conditional updates require the expected prior phase
and matching manifest digest.

## `codex_session_transfer_artifacts`

Ordered immutable evidence for one transfer.

| Field | Meaning |
| --- | --- |
| `operation_id` FK | Owning transfer |
| `ordinal` | Ancestor-first stable order |
| `thread_id` | Canonical rollout thread UUID |
| `parent_thread_id` | Referenced predecessor, if any |
| `relation` | `parent`, `forked_from`, or `leaf` |
| `relative_path` | Validated Codex sessions-relative path |
| `size_bytes` | Exact bounded byte count |
| `sha256` | Source and target content digest |
| `target_verified_at` | Per-entry install/reopen proof |
| `last_needed_at` | Cleanup retention clock |

Primary key `(operation_id, ordinal)`; unique `(operation_id, thread_id)` and
`(operation_id, relative_path)`. Rows become immutable after manifest phase.

## State transitions

`claimed → manifested → staging → verified → affinity stop/placement/restart`

Any pre-verification phase may move to `failed`; that transition never changes
execution or placement. A retry with the same identity may resume a nonterminal
phase or replay `verified`. Different context/manifest is a conflict.

Cleanup candidates require age plus absence of an active/recoverable affinity
operation or execution reference. Deleting filesystem content does not delete
evidence immediately; evidence records the cleanup result for audit/idempotency.
