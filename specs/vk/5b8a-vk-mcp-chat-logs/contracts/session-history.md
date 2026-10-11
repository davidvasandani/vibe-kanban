# Contract: session chat history

## HTTP: `GET /api/sessions/{session_id}/history`

Query (all optional):

| name | format | default | validation |
|---|---|---|---|
| `since` | RFC 3339 | — | 400 if unparseable |
| `until` | RFC 3339 | — | 400 if unparseable, or if `since > until` |
| `query` | string | — | trimmed; empty = no filter |
| `roles` | csv | all | same parser as `/messages` |
| `execution_id` | uuid | — | 400 if not a non-dropped coding-agent turn of this session |
| `limit` | int | 50 | clamped to 1..=200 |
| `offset` | int | 0 | — |
| `order` | `asc`/`desc` | `asc` | 400 (serde) on other values |

Responses:
- `200` `ApiResponse<SessionHistoryResponse>` (see `../data-model.md`)
- `400` validation error (`ApiError::BadRequest`)
- `404` unknown session (`load_session_middleware`)

A session with zero coding-agent turns returns 200 with no messages
(unlike `/messages`, which 404s because it has no "latest turn" to report).

## MCP tool: `search_session_messages`

Input:

```json
{
  "session_id": "uuid (required)",
  "since": "RFC 3339 (optional)",
  "until": "RFC 3339 (optional)",
  "query": "string (optional)",
  "roles": "user,assistant,system,tool (optional)",
  "execution_id": "uuid (optional)",
  "limit": 50,
  "offset": 0,
  "order": "asc | desc"
}
```

Output: the HTTP response body's `data`, unchanged.

Errors: the scope violation ("outside the configured MCP scope") and backend
400/404 messages are surfaced as tool errors, like the other session tools.
