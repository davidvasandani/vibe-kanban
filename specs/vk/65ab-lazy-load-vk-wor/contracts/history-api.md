# Contract: Workspace conversation history pages

## Request

`GET /api/sessions/{session_id}/conversation-history?limit=100&before={cursor}`

- `limit` is optional, defaults to 100, and is capped at 200.
- `before` is absent for the latest page and is the prior response's opaque
  cursor for older pages.
- Existing authentication/authorization must prove access to the session's
  workspace before cursor decoding can reveal data.

## Success response

```json
{
  "success": true,
  "data": {
    "entries": [
      {
        "execution_process_id": "uuid",
        "entry_index": 42,
        "revision": 57,
        "entry": { "entry_type": { "type": "assistant_message" }, "content": "..." }
      }
    ],
    "next_cursor": "opaque-base64url",
    "has_more": true,
    "live_watermarks": {
      "running-process-uuid": 57
    }
  }
}
```

Entries are chronological within the returned page. Their stable frontend key
is `{execution_process_id}:{entry_index}`.

## Errors

- `400`: malformed cursor or invalid limit.
- `403/404`: caller cannot access the route session (without leaking whether a
  cross-scope cursor target exists).
- `409`: cursor generation is stale; client should refresh the bounded latest
  page.
- `425`: legacy transcript materialization is still preparing; client keeps the
  workspace usable and may retry using server-provided backoff guidance.
- `500/503`: materialization failed or capacity is temporarily unavailable;
  existing loaded history remains usable and the request may be retried.

## Live stream extension

Each normalized live patch is associated with a monotonic process-local
`revision`. The live subscription can resume strictly after a supplied
watermark, or emits an explicit resnapshot-required message if that revision is
no longer available. A page snapshot's `live_watermarks` is the deduplication
boundary; revisions at or below it are already represented by the snapshot.
