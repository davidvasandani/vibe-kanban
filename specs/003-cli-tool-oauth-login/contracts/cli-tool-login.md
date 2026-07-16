# Contract: CLI tool authentication and login

## Status response

`GET /api/cli-tools` retains its current response and adds per tool:

```json
{
  "login_supported": true,
  "auth_state": "unauthenticated",
  "auth_message": null
}
```

`auth_state` is one of `authenticated`, `unauthenticated`, `unknown`, or
`unsupported`. Messages are bounded, human-readable, and contain no command
output or credentials.

## Login WebSocket

`GET /api/cli-tools/{id}/login/ws?cols=80&rows=24`

The endpoint uses the existing signed/machine-aware WebSocket transport.

Client messages:

```json
{"type":"input","data":"<base64 bytes>"}
{"type":"resize","cols":100,"rows":30}
{"type":"cancel"}
```

Server messages:

```json
{"type":"output","data":"<base64 bytes>"}
{"type":"exit","outcome":"succeeded","exit_code":0}
{"type":"status","tool":{}}
{"type":"error","code":"session_conflict","message":"Login is already active"}
```

Error codes: `tool_unavailable`, `login_unsupported`, `session_conflict`,
`spawn_failed`, `timed_out`, `cancelled`, `command_failed`, and
`verification_failed` (the command exited successfully but the independent
probe did not confirm authentication).

The server chooses executable/arguments from the catalog; the client cannot
submit commands. On command exit it probes independently, emits final status,
then closes cleanly. Disconnect cancels and cleans up the process.
