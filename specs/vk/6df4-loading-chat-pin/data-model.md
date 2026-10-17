# Data Model: Historical Materialization Coordination

No database or generated API type changes are required.

## Persistent entities

### Normalized log sidecar

Existing entity, keyed by execution process path:

- `version`: cache schema/normalizer compatibility version;
- `entry_count`: settled normalized entry count;
- `truncated`: whether bounded reconstruction omitted older messages;
- `entries`: complete ordered settled entries.

Only atomic write completion makes this reusable. Invalid version, count,
shape, or truncated file is a miss.

## Ephemeral entities

### Historical materialization cell

- key: `execution_id: Uuid`;
- ownership: one async mutex guard;
- retention: registry holds only a weak reference; an active/waiting operation
  holds the strong reference;
- durable status: none. The sidecar is the only success record.

### Historical normalization lifetime

- execution ownership guard;
- global capacity permit;
- normalizer and completion-task abort handles;
- completion/cancellation diagnostic state.

Dropping the lifetime aborts unfinished tasks, releases capacity, and lets a
waiter retry. Successful stream completion atomically writes the sidecar before
the lifetime is released.

## State transitions

```text
cache hit -> replay
cache miss -> wait for execution cell
cell acquired -> cache recheck
  hit -> replay
  miss -> wait for global capacity -> normalize -> stream
    complete + atomic write -> release -> waiter replays cache
    drop/failure -> abort/release -> waiter retries as leader
```
