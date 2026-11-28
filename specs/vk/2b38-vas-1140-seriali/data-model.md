# Data model — VAS-1140 (all in-memory)

## MirrorKey
`(dest: PathBuf, branch: String)`

## MirrorSlot
| field | type | meaning |
|---|---|---|
| in_flight | bool | a leader task owns the key |
| rerun | bool | a trigger arrived during the flight |
| failure | Option<FailureState> | last failure, cleared on success |

## FailureState
| field | type |
|---|---|
| kind | `SourceMirrorFailureKind` = `deterministic` \| `transient` \| `timeout` |
| message | String (bounded, about 2 KB) |
| attempts | u32 (consecutive failures) |
| last_attempt_at | DateTime<Utc> |
| next_attempt_at | DateTime<Utc> (backoff) |
| failed_tip | Option<String> (store commit at failure) |

Backoff: `min(cap, 60 s × 2^(attempts−1))`. The cap is 30 min for transient
and timeout failures and 6 h for deterministic ones. A deterministic failure
also requires `store tip != failed_tip`.

## SourceMirrorFailure (wire, ts-rs)
`{ kind, message, attempts, last_attempt_at, next_attempt_at }`

## Registration (utils::worktree_linkage)
`{ admin_dir, worktree_path, head_branch: Option<String>, locked: bool }`.
Its state is one of `Live` | `Stale(reason)` | `Indeterminate(reason)`.
