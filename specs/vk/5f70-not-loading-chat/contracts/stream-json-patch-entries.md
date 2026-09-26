# Contract: `streamJsonPatchEntries(url, opts)`

## Options (additions)

- `idleTimeoutMs?: number`: when set, the stream fails if no message arrives
  for this long. The timer starts when the stream is created and is reset by
  every message. Unset means no idle bound (live streams).

## Settlement

Exactly one of `onFinished(entries)` or `onError(err)` is called per stream,
unless the caller closes first, in which case neither is.

| Event (first to occur)                         | Outcome                  |
|------------------------------------------------|--------------------------|
| `{"finished": ...}` message                    | `onFinished(entries)`    |
| socket `close` without prior `finished`        | `onError(Error)`         |
| socket `error`                                 | `onError(Error or Event)` |
| unparseable message                            | `onError(err)`           |
| `openLocalApiWebSocket` rejects                | `onError(err)`           |
| `idleTimeoutMs` elapses without a message      | socket closed, `onError(Error)` |
| caller `close()`                               | nothing                  |

Any signal after settlement is ignored. `onEntries` and `onChange` subscribers
are unaffected.
