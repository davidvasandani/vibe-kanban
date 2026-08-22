# Prior Knowledge: complete MCP message reads

Searched `wiki/`, `docs/knowledge-base/`, the MCP crate guide, and the existing
message-route implementation for normalized conversation history, MCP tools,
limits, ordering, and historical reconstruction.

## Relevant findings

1. `list_recent_messages` already establishes the desired public contract:
   session targeting resolves the latest non-soft-deleted `CodingAgent`
   execution; execution targeting reads that turn directly; both authorize the
   owning session's workspace before returning data.
2. The MCP result and the UI use the same normalized-log pipeline through
   `ContainerService::normalized_entries`. A new all-messages tool should reuse
   this settled projection rather than parse raw logs or add storage.
3. The server response builder currently materializes every normalized entry,
   applies role and empty-text filtering, then tails the vector according to a
   clamped `limit` (default 20, maximum 100). The HTTP cap—not the MCP schema—is
   what prevents `list_recent_messages` from returning a complete turn.
4. Normalized entries have lifecycle-sensitive patch semantics. Earlier events
   can be replaced or removed, so raw JSONL cannot be safely reverse-read or
   sliced to synthesize a complete conversation. Materialization must precede
   role filtering and response selection.
5. Completed executions normally replay an atomically materialized normalized
   sidecar. For legacy cache misses, reconstruction is single-flight and
   capacity-bounded. The new read path must stay on this machinery to preserve
   those performance and cancellation guarantees.
6. Historical cache misses deliberately normalize at most the newest 2,000 raw
   normalizable messages and inject an omission notice when earlier raw records
   are dropped. Therefore `list_all_messages` can promise every message in the
   normalized projection available to Vibe Kanban, but cannot retroactively
   promise lossless raw-log history for legacy oversized executions. Freshly
   completed executions cache their full in-memory normalized history.
7. Stable response identity is already `{execution_id}:{entry_index}`, output
   order is chronological, individual text is capped at 4,000 characters, and
   `final_message` is calculated before role filtering. These semantics should
   remain identical between recent and all-message tools.
8. The MCP router has an explicit orchestrator tool-name regression test. The
   new read-only scoped tool must be added to that expected set.

## Planning consequences

- Add an explicit all-messages query mode rather than abusing a sentinel limit
  or raising/removing the recent-message cap.
- Keep one response builder and parameterize only bounded-tail versus complete
  selection so filtering, ordering, truncation, status, and final-message logic
  cannot drift.
- Share MCP target resolution and authorization between the two tools.
- Test with more than 100 normalized entries to prove the new mode crosses the
  existing cap while recent mode remains bounded.
- Document the normalized-projection boundary and the existing legacy-history
  cap honestly.
