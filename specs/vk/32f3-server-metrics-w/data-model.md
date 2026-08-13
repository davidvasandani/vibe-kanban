# Data Model: Server Metrics Low-Disk Warnings

## DiskAlertThresholds

| Field | Type | Default | Rule |
| --- | --- | --- | --- |
| `warning_free_percent` | finite non-negative percent | 10 | warning when free percent `<` value |
| `warning_free_bytes` | non-negative bytes | 5 GiB | warning when available `<` value |
| `critical_free_percent` | finite non-negative percent | 2 | critical when free percent `<` value |
| `critical_free_bytes` | non-negative bytes | 1 GiB | critical when available `<` value |

Critical percent/bytes must not exceed the matching warning threshold.

## LowDiskFilesystemObservation

Node-scoped immutable request evidence: device, filesystem type, mountpoint,
total/used/available bytes, use percentage, severity, and captured timestamp.
All numeric facts are validated and recomputed against effective server
thresholds; the client does not choose its severity authoritatively.

## Low-disk issue identity

Stored in `issues.extension_metadata.low_disk`:

- `kind`: fixed `server_low_disk`
- `node_id`: stable metrics node UUID
- `hostname`: display evidence, not identity

Logical unique-open key: `(project_id, kind, node_id)`. Terminal means the
joined project status name is `Done`, `Cancelled`, or `Canceled`, case-insensitively.

## ResolveLowDiskIssueResult

- `issue`: canonical issue record
- `created`: whether this call inserted it
- `txid`: transaction identity used by frontend convergence
