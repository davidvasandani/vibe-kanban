# HTTP Contract: execution/session messages

Existing endpoints:

- `GET /api/execution-processes/{execution_id}/messages`
- `GET /api/sessions/{session_id}/messages`

Query parameters:

- `limit` (optional integer): recent mode, default 20, clamped to 1..100.
- `roles` (optional comma-separated roles): unchanged.
- `all` (optional boolean, default false): when true, return every filtered
  entry in the available settled normalized projection and ignore `limit`.

Response shape is unchanged. In all mode, `has_more` is always false. Existing
clients that omit `all` retain the current bounded behavior.
