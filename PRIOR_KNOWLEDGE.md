# Prior Knowledge: Timestamp the Workspace Chat Log

Task: `vk/a22f-time-stamp-chat`

## Relevant knowledge-base findings

### Normalized history is the display source of truth

From `lazy-loading-normalized-conversation-history.md`: workspace chat is built
from final normalized entries keyed by execution process and entry index. The
frontend preserves stable semantic identity as `{process_id}:{entry_index}`, and
live updates may replace an existing entry without changing its identity. A
timestamp change must therefore remain metadata on the existing normalized row;
it must not introduce a separate row or alter keys, ordering, paging, or
virtualization anchors.

### Entry timestamps originate in executor normalization

From `claude-log-normalization.md`: executor processors emit add and replace
patches for `NormalizedEntry` values, and later lifecycle events can refine the
same entry in place. Persisted `NormalizedEntry.timestamp` is the authoritative
event time when present. The UI should consume this value rather than stamp
entries on receipt, because replay and live delivery times are not event times.

### One visible row may summarize repeated activity

From `collapsing-repeated-log-entries.md`: repeated system events and narrowly
eligible commands may be compacted by replacing one normalized entry. The
timestamp attached to that final entry is consequently the correct timestamp
available for the summarized visible row. Rendering timestamps must work for
aggregated frontend rows as well as single normalized rows and must not undo
server-side compaction.

### Chat row geometry and identity are sensitive

From `edge-triggered-chat-scroll-intents.md`: chat scrolling relies on stable
semantic row identities and invariant geometry inside a virtualized scroll
surface. Timestamp UI should be compact, should not create new semantic rows,
and should avoid conditional structure that destabilizes expansion or scroll
anchoring. Tests should protect existing keys and row composition, while a
browser check should cover the visual result.

## Implications for this task

1. Keep timestamp metadata on each existing entry/row; never synthesize a
   standalone timeline event.
2. Prefer `NormalizedEntry.timestamp`; retain execution-process `created_at` as
   the authoritative parent fallback because many executor normalizers omit
   event timestamps, including for persisted entries.
3. Preserve patch keys, aggregation, ordering, and virtualization inputs.
4. Treat invalid/missing timestamps as absent instead of substituting the
   browser's current time.
5. Use a small reusable presentation primitive so messages, events, actions,
   and grouped rows share formatting and accessibility behavior.
