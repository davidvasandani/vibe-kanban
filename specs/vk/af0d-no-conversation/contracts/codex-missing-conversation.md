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
   - `invalid paginated history lineage for <requested-uuid>: missing source rollout`;
4. any error data is absent/null rather than contradicting the classification.

Case and punctuation are part of each known template. A different UUID, extra
suffix, broad `thread not found`, different paginated-lineage suffix, or any
other code is not eligible.

### Paginated history lineage errors (Codex 0.154+)

Codex 0.154 introduced paginated history, where older turns may be stored in
separate rollout files linked via a `history_base` field. When `thread/fork`
resolves a thread whose `history_base` points to a missing ancestor rollout, it
returns `invalid paginated history lineage for <uuid>: missing source rollout`.

This typically occurs in cluster deployments when a conversation was started on
one worker but a follow-up runs on a different worker (or from a different
`CODEX_HOME`) where the ancestor rollout was never transferred. The recovery
behaviour is the same: start a replacement thread in the same workspace.

## Outcome

```text
fork succeeds -> register forked ID -> start prompt once
fork missing  -> start thread -> register replacement ID -> start prompt once
fork other    -> return original structured error
```

The replacement is a new conversation in the same workspace; no missing
private context is reconstructed.
