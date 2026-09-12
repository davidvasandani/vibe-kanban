# Codex Missing-Conversation Recovery Contract

## Input

- Operation: normal Codex chat follow-up.
- Requested source ID: valid Codex thread UUID.
- `thread/fork` result: success or structured JSON-RPC error.

## Classification

Recovery is eligible only when all are true:

1. the response is from `thread/fork`;
2. JSON-RPC code is invalid request (`-32600`);
3. the complete normalized message is one of:
   - `no rollout found for thread id <requested-uuid>`;
   - `No conversation found with session ID: <requested-uuid>`;
4. any error data is absent/null rather than contradicting the classification.

Case and punctuation are part of each known template. A different UUID, extra
suffix, broad `thread not found`, or any other code is not eligible.

## Outcome

```text
fork succeeds -> register forked ID -> start prompt once
fork missing  -> start thread -> register replacement ID -> start prompt once
fork other    -> return original structured error
```

The replacement is a new conversation in the same workspace; no missing
private context is reconstructed.
