# Contract: Historical Materialization

## External WebSocket contract

The existing normalized execution-log WebSocket remains unchanged:

- route and signing behavior are unchanged;
- messages remain existing `LogMsg::JsonPatch` frames followed by
  `LogMsg::Finished`;
- a cache hit and a cold reconstruction are semantically equivalent;
- running processes continue to use their in-memory live stream.

No client, generated type, or frontend migration is required.

## Internal coordination contract

Given execution ID `E`:

1. A valid durable cache is returned immediately.
2. At most one process-local reader owns reconstruction for `E`.
3. Other readers of `E` await that ownership boundary without consuming global
   reconstruction capacity.
4. After ownership is obtained, the durable cache is checked again.
5. Only a still-missing leader may enter the global capacity queue.
6. Ownership remains held until the output stream completes or is dropped.
7. Successful completion attempts one atomic sidecar publication before
   releasing ownership.
8. Failure/drop publishes no in-memory success and releases ownership for
   retry.
9. Coordination for execution `E` does not block cache hits or ownership for a
   different execution ID.

## Diagnostic contract

Every event may include execution ID, safe counts, and elapsed durations. It
must not include prompts, raw log lines, normalized entries, patches, executor
environment, tokens, or credentials.
