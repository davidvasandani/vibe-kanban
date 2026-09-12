# Data model: Lazy-load workspace chat history

## Normalized transcript entry

Durable process-local materialized state.

| Field | Meaning |
| --- | --- |
| `execution_process_id` | Owner process; part of stable identity |
| `entry_index` | Absolute `/entries/{index}` identity within the process |
| `revision` | Monotonic process-local patch revision that produced this state |
| `entry` | Final normalized entry payload |
| `updated_at` | Diagnostic/cache freshness timestamp |

Primary identity is `(execution_process_id, entry_index)`. Add/replace upserts
that identity; remove deletes it. Revision only moves forward.

## Transcript materialization

| Field | Meaning |
| --- | --- |
| `execution_process_id` | One record per normalizable process |
| `schema_version` | Cache encoding/normalizer materialization version |
| `last_revision` | Highest durably applied patch revision |
| `complete` | Whether the process reached terminal normalized state |
| `source_fingerprint` | Detects stale cache against persisted raw source |

State transitions:

1. `missing` → `building` when live execution begins or legacy replay is claimed.
2. `building` applies ordered add/replace/remove operations atomically.
3. `building` → `complete` only after normalized completion is durable.
4. Fingerprint/schema mismatch invalidates the materialization and returns it to
   `building`; readers never mix generations.

## Conversation page

| Field | Meaning |
| --- | --- |
| `entries` | Chronological materialized entries with stable process/index ids |
| `next_cursor` | Opaque boundary for the immediately preceding page, or null |
| `has_more` | Explicit earlier-history availability |
| `live_watermarks` | Highest snapshot revision for each running process |

## History cursor (server-private decoded form)

| Field | Meaning |
| --- | --- |
| `version` | Cursor schema discriminator |
| `session_id` | Prevents cross-session reuse |
| `before_created_at` | Process ordering boundary |
| `before_process_id` | Deterministic timestamp tie-breaker |
| `before_entry_index` | Entry boundary inside the process |
| `generation` | Materialization generation/snapshot consistency |

## Frontend history state

| Field | Meaning |
| --- | --- |
| `processes` | Loaded materialized entries grouped for existing derivation |
| `nextCursor` | Next earlier page cursor |
| `hasEarlier` | Server-provided exhaustion state |
| `isLoadingEarlier` | Single-flight guard/UI state |
| `loadEarlierError` | Recoverable page error |
| `scopeGeneration` | Rejects late results after workspace/session changes |
| `liveWatermarks` | Snapshot/live deduplication boundary |
