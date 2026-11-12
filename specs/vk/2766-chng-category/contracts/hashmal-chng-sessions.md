# Hashmal MCP tool `hashmal_chng_sessions` (platform-ops PR)

Request: `tools/call` `{name: "hashmal_chng_sessions", arguments:
{key: "CSEQ-123", session_id?: "<jsonl stem>", limit?: 1..50 (20)}}`

Result `structuredContent`:
```json
{
  "key": "CSEQ-123",
  "tracked": true,
  "summary": "…", "url": "https://…/browse/CSEQ-123",
  "manual": false, "cleared": false,
  "sessions": [{"session_id": "…", "title": "…", "cwd": "…",
                "last_active": "2026-11-01T12:00:00+00:00", "running": false}],
  "transcript": {"session_id": "…", "total_turns": 120, "returned": 20,
                 "turns": [{"role": "user|assistant|tool|question", "text": "…",
                            "truncated": false, "tool": null, "ts": "…"}]}
}
```
- `tracked: false` → ticket not in Hashmal's CHNG mirror; `sessions: []`,
  `transcript: null`. No transcript scan on request.
- `transcript` is for `session_id` when given, else the primary session;
  null when there are no sessions.
- A `session_id` not among the ticket's sessions → tool error (`isError`).
- Read-only; key must match `^[A-Z][A-Z0-9_]*-[0-9]+$`.
