# Research: `list_all_messages`

## Decision: explicit selection mode

Use a typed bounded-tail/all selection rather than `usize::MAX`, zero, or a
raised global cap. This keeps the established recent contract intact and makes
complete-mode intent visible at every server call site.

## Decision: reuse normalized entries

`ContainerService::normalized_entries` is the only correct source because
executor logs are lifecycle-sensitive patch streams. Reverse-reading or slicing
raw JSONL can miss prior adds, replacements, removals, and normalizer state.

## Decision: preserve reconstruction bounds

The all-message tool removes the 100-entry response cap, not the existing
2,000-raw-message safety bound applied to oversized legacy cache misses. That
bound prevents a single historical request from exhausting server memory and
emits an explicit omission entry, while fresh completed turns store their full
normalized materialization.

## Alternatives rejected

- **Set `limit` to a very large value:** still clamped by the server and hides
  intent behind a magic number.
- **Remove the 100-message cap globally:** changes `list_recent_messages` and
  removes its safety/latency contract.
- **Add pagination:** valuable for a future durable cross-execution history
  API, but does not satisfy the requested one-call tool and requires cursor and
  materialized-view work beyond this task.
- **Read the normalized websocket directly from MCP:** duplicates projection
  assembly and complicates a settled read with a streaming transport.

No external research or new dependency is required.
