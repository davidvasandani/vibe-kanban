# Contract — coordinator handling of `ExecutionEventPayload::Structured`

Input: `json: String` from a worker event batch.

| `json` shape | Coordinator action | Reaches agent stdout? |
|---|---|---|
| Valid `LogMsg` (externally tagged, e.g. `{"Stderr":"…"}`) | `store.push(msg)` plus final-assistant-state tracking (unchanged) | only if it is `LogMsg::Stdout` |
| `{"cancellation_phase": "<phase>"}` | `tracing::debug!` only | **no** |
| `{"worker_error": "<reason>"}` | `LogMsg::Stderr("Worker error: <reason>")` | **no** |
| `{"stream_error": "<reason>"}` | `LogMsg::Stderr("Worker output stream error: <reason>")` | **no** |
| anything else (including invalid JSON) | `LogMsg::Stderr("Unrecognized worker event: <json>")` | **no** |

Invariant: after this change, only `Stdout` events and `LogMsg::Stdout`
values write to the agent's stdout stream.
