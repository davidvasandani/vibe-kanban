# Data Model: Pipeline Seed State

## Seed manifest

Private file in the pipelines directory (not a `*.toml`).

| Field | Type | Rules |
| --- | --- | --- |
| `version` | unsigned integer | Must equal the supported manifest format version. |
| `bundled` | array of strings | Deterministic current `BUNDLED` filenames; entries must be safe basename TOML filenames. |

## State transitions

| Prior directory state | Effective known set | Result |
| --- | --- | --- |
| No pipeline TOMLs | Empty | Create all missing current bundled files, then record current set. |
| TOMLs, no manifest | Legacy baseline | Create missing current entries outside baseline, then record current set. |
| Valid manifest | Manifest `bundled` set | Create missing current entries outside recorded set, then record current set. |
| Invalid manifest | Unknown | Return error; do not write pipelines or replace manifest. |

An absent filename already in the effective known set represents a preserved
user deletion. A present target is never overwritten regardless of state.
