# Data Model: The chat panel always finishes loading

No persisted entities, API payloads, or generated types change.

The only new state is transient and per stream inside `streamJsonPatchEntries`:

| Field      | Type                    | Meaning                                                   |
|------------|-------------------------|-----------------------------------------------------------|
| `settled`  | boolean                 | `onFinished` or `onError` has fired; later signals ignored |
| idle timer | timeout handle or null  | Armed only when `idleTimeoutMs` is set; reset per message   |
