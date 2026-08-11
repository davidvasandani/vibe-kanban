# Data Model: Parallel Pipeline Refresh

No public data model changes.

## Embedded migration data

| Value | Representation | Rule |
| --- | --- | --- |
| Current parallel pipeline | Existing embedded asset | Written for fresh seed/reset and as migration destination. |
| Previous parallel pipeline | Private compile-time string | Used only as an exact byte-match migration source. |

## State transitions

| On-disk state | Result |
| --- | --- |
| Missing and not previously known | Seed current default through existing logic. |
| Missing and already known | Preserve deletion. |
| Exact previous bundled bytes | Atomically replace with current default. |
| Current bundled bytes | No change. |
| Any other bytes | Preserve as user-customized or unknown. |

The existing filename-set seed manifest remains unchanged.
