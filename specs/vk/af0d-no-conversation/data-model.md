# Data Model

No new persisted entities or schema changes are introduced.

The existing relationship remains:

```text
Vibe Session -> CodingAgentTurn -> agent_session_id (Codex thread UUID)
```

On recovery, normal thread registration emits the replacement UUID. Existing
execution-log handling updates the current coding-agent turn, and
`find_latest_session_info` selects that replacement for the next follow-up.
The previous missing UUID remains historical evidence on its older turn.
