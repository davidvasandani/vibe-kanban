# Codex Missing-Conversation Recovery Contract

## Input

- Operation: normal Codex chat follow-up.
- Requested source ID: valid Codex thread UUID.
- `thread/fork` result: success or structured JSON-RPC error.

## Classification

Fork rejection is classified into two categories:

### Conversation Missing

Replacement eligible when all are true:

1. the response is from `thread/fork`;
2. JSON-RPC code is invalid request (`-32600`);
3. the complete normalized message is one of:
   - `no rollout found for thread id <requested-uuid>`;
   - `No conversation found with session ID: <requested-uuid>`;
4. any error data is absent/null rather than contradicting the classification.

### Lineage Unusable

Resume eligible when all are true:

1. the response is from `thread/fork`;
2. JSON-RPC code is invalid request (`-32600`);
3. the message matches:
   - `invalid paginated history lineage for <requested-uuid>: missing source rollout`;
4. any error data is absent/null.

This occurs with Codex 0.154+ when a rollout has `history_mode=paginated` but no
`history_base` field, and the expected ancestry rollout was never written.

### Neither

Case and punctuation are part of each known template. A different UUID, extra
suffix, broad `thread not found`, different paginated-lineage suffix, or any
other code is not eligible for recovery.

## Outcome

```text
fork succeeds                              -> register forked ID -> start prompt once
fork lineage unusable + local leaf exists  -> resume thread -> start prompt once
fork lineage unusable + no local leaf      -> start thread -> register replacement ID -> start prompt once
fork conversation missing                  -> start thread -> register replacement ID -> start prompt once
fork other                                 -> return original structured error
```

When resuming a thread with unusable lineage, the conversation text already
present in the local leaf is preserved. The replacement path is taken only when
the rollout file is genuinely absent.

The replacement is a new conversation in the same workspace; no missing
private context is reconstructed.
