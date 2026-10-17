# Contract: execution process log snapshots

## `GET /api/execution-processes/{id}/normalized-logs`
## `GET /api/execution-processes/{id}/raw-logs`

Both are host-scoped through `/api/host/{hostId}/…` and the remote relay, like
every other local API route. Both pass through
`load_execution_process_middleware`, so an unknown id gets that middleware's
404.

**200** → `ApiResponse<ExecutionProcessLogSnapshot>`

```json
{ "success": true,
  "data": { "entries": [ { "type": "NORMALIZED_ENTRY", "content": { … } } ],
            "complete": true } }
```

- `normalized-logs`: the entries are those the `normalized-logs/ws` replay
  produces. They come from the materialized sidecar when one exists, and
  otherwise from one bounded historical normalization (newest 2,000 messages,
  with an omission notice as a `STDOUT` entry). That normalization writes the
  sidecar, so the next read is cheap.
- `raw-logs`: `{type: "STDOUT"|"STDERR", content}`, in the order logged.
- `complete: false` when a live `MsgStore` exists (the entries are a snapshot
  of retained history), when the process status is `running`, or when the
  settled source ended without `Finished`. The client must not treat these
  entries as final.
- A process with no logs at all returns `entries: []` with
  `complete = (status != running)`.
- The server keeps building after the client disconnects. The response may be
  abandoned, but the sidecar is still written.

**Client obligations** (constitution XL and XLIV):
- an `AbortController` deadline of 30 s;
- timeout, abort, non-2xx, `success: false` and malformed JSON all mean
  failure;
- settle exactly once;
- on `complete: false`, fall back to the matching `…/ws` with the idle
  timeout.

## Changed: `GET /api/execution-processes/stream/session/ws`

The server is unchanged. Clients always send `show_soft_deleted=true` and
filter `dropped` locally.

## Changed: `agentsApi.getDiscoveredOptionsStreamUrl`

When `sessionId` is set, the URL contains only `executor` and `session_id`.
The server already ignores `repo_id` and uses `workspace_id` only for a
consistency check in that case.
