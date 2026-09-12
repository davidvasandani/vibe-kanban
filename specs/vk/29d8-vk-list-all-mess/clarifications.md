# Clarifications: `list_all_messages`

## 1. What does “all” include?

**Decision:** Return all entries in Vibe Kanban's settled normalized projection
for the selected execution. Preserve the existing legacy reconstruction safety
bound and its explicit omission notice.

Removing the raw-history bound would expand this small MCP capability into a
different storage/materialization project and conflict with the constitution's
bounded historical-view requirement. Fresh completed executions already cache
their full normalized in-memory history; oversized legacy cache misses remain
truthfully partial rather than risking unbounded reconstruction.

## 2. What happens when both target identifiers are supplied?

**Decision:** Preserve `list_recent_messages`' execution-first behavior.

This makes the new tool a direct unbounded counterpart to the existing reader
and avoids an unnecessary compatibility difference. The schema descriptions
will state that `execution_id` takes precedence when both are present.

## Remaining questions

None.
