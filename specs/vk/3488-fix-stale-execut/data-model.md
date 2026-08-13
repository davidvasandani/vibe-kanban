# Data Model: Execution Status Reconciliation

No persistent schema changes are required.

The existing `ExecutionProcessStatus` domain remains authoritative:

- `running`: active and cancellable;
- `completed`, `failed`, `killed`, `interrupted`, `indeterminate`: non-running
  composer states.

The streamed client state remains a map keyed by execution process ID. A full
snapshot replaces the map; later patches upsert or remove individual keys.
