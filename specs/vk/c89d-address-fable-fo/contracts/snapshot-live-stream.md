# Contract: Lossless Snapshot + Live Stream

1. Acquire the bounded live receiver.
2. Await and construct the authoritative snapshot.
3. Emit the full replacement snapshot.
4. Emit `Ready`.
5. Drain buffered and then future relevant live messages in publication order.
6. On receiver lag, emit an `io::Error` and terminate.
7. The WebSocket adapter sends a retryable error close with a resnapshot reason.

Reducing the emitted messages must produce the latest authoritative keyed state.
Tests pause between steps 1 and 2 and publish a terminal transition.
