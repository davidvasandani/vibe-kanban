# Codex rollout transfer across workers

Tags: `c8a9-transfer-codex-r`, `vk/af0d-no-conversation`

## Transfer artifacts, not executor homes

Codex continuation depends on immutable JSONL rollout files under
`CODEX_HOME/sessions`; `thread/fork` must be able to resolve the requested
thread on the target worker. Transfer only the canonical leaf rollout and its
validated ancestry. Never synchronize the complete executor home: it also
contains credentials, configuration, logs, and mutable SQLite state.

Resolve identity from the active execution's own coding-agent turn. A
session-wide “latest” lookup can select an older execution before the current
turn has persisted its thread ID, silently resuming stale history. Refuse the
migration until the active execution has a canonical UUID.

## Quiesce before hashing and stop only after verification

Rollout files can still change while Codex runs. The coordinator therefore
owns this order:

1. claim a durable affinity operation and persist the selected target worker;
2. quiesce the source process group under an operation-specific lease;
3. resolve, hash, transfer, and fully verify the lineage on the target;
4. durably record verification evidence;
5. stop the source, commit placement, and create the deterministic follow-up.

Any pre-stop failure resumes the source and leaves placement unchanged. A
watchdog eventually resumes an abandoned quiescence, but explicit compensation
is still required at every known failure boundary. Persisting the selected
target matters for automatic affinity: scheduler inputs can change between an
initial attempt and crash recovery.

## Make staging bounded, contained, and idempotent

Treat worker APIs as signed, context-bound capabilities. Validate operation,
workspace, source/target worker, execution, requested leaf, manifest digest,
size, and checksum on every phase. Set HTTP body limits large enough for the
base64-expanded artifact while retaining stricter decoded file and lineage
limits.

Derive destinations from canonical metadata, not caller paths. Reject absolute
paths, traversal, symlinks in every path component, non-regular files,
oversized data, UUID substitution, cycles, and ambiguous distinct ancestry
fields. Install through private same-directory temporary files and an atomic
no-clobber link. Identical existing content is a successful retry; different
content is a conflict. On post-install verification failure, remove the
destination only when it still has the temporary file's inode, so a concurrent
replacement is never deleted.

## Recover by durable phase

Retries before final verification repeat manifest discovery and staging;
operation/thread-specific partial files may be replaced safely. Retries after
durable verification re-read and re-hash the complete target manifest before
stopping or dispatching. The continuation execution uses the affinity
operation ID, preventing duplicate starts when a response is lost.

Record safe evidence—IDs, phases, sizes, digests, and categories—but never
rollout contents. Surface transfer failures as a specific affinity outcome so
operators do not have to diagnose a later generic `thread/fork` I/O error.

## Retain conservatively

Vibe-owned receipts distinguish transferred files from native Codex files.
Abandoned partials can be removed after 24 hours in bounded periodic passes.
Verified artifacts become age-eligible after 30 days, but age alone is not
proof that an idle resumable session no longer references them. A worker may
delete verified lineage only when coordinator state proves there is no active
execution, resumable session, or recoverable migration reference; otherwise it
must retain the file.

## Recover a genuinely absent rollout at the conversation boundary

Even outside an affinity migration, a persisted Vibe Kanban turn can reference
a Codex thread whose rollout no longer exists. A normal chat follow-up should
try `thread/fork` first, then start a replacement thread in the same workspace
only when the structured app-server error proves that exact requested UUID is
absent. Register the replacement through the ordinary session-ID path before
starting the turn so later follow-ups naturally continue it.

Preserve JSON-RPC code, message, and data at the client boundary. Match the
complete, pinned missing-rollout response (plus any exact production-observed
legacy form) rather than `contains("not found")`; the same invalid-request code
and similar phrases cover archived sessions, malformed IDs, and operations on
unloaded live threads. Every nonmatching error remains fail-loud. This recovery
keeps the Vibe workspace usable but cannot reconstruct lost Codex-private
context, and context-dependent operations such as review or compaction should
not silently become fresh conversations.

### Known missing-conversation error formats

The following `thread/fork` error messages (JSON-RPC code `-32600`, null data)
indicate a genuinely missing conversation where replacement is required:

1. `no rollout found for thread id <uuid>` — the leaf rollout file is absent.
2. `No conversation found with session ID: <uuid>` — legacy session lookup
   failed.

All require the `<uuid>` in the message to match the exact thread ID requested
by Vibe Kanban. A mismatch, wrong JSON-RPC code, or non-null error data means
the error is not eligible for recovery.

## Resume threads with unusable lineage when the leaf exists

Codex 0.154+ introduced paginated history mode and stricter lineage validation.
A `thread/fork` request may fail with `invalid paginated history lineage for
{uuid}: missing source rollout` even when the leaf rollout file is present. This
occurs when the session metadata has `history_mode=paginated` but no
`history_base` field, and the expected ancestry rollout was never written.

When a fork fails due to lineage issues:

1. Check whether the leaf rollout file exists locally for the requested thread.
2. If the leaf exists, call `thread/resume` with the original thread ID to load
   the thread from the on-disk rollout into the app-server's memory. The
   `thread/resume` method is designed to load an existing thread by thread_id
   without requiring full ancestry resolution like `thread/fork` does.
3. If `thread/resume` succeeds, the resumed thread is used for the turn. The
   conversation text from the leaf is preserved.
4. If `thread/resume` also fails (e.g., the same lineage error or other issue),
   or if the leaf is absent, fall back to the replacement-thread behavior.

This two-stage approach (`thread/fork` → `thread/resume` → replacement) gives
the best chance of preserving conversation context. The replacement-thread
fallback remains as the last resort for genuinely irrecoverable rollouts.

### Protocol methods tried for lineage-unusable leaves

The following methods were evaluated for loading paginated leaves with no
`history_base`:

1. **`thread/fork`**: Fails with "invalid paginated history lineage" because it
   requires full ancestry resolution for paginated history mode.
2. **`thread/resume`**: Loads the thread from disk by thread_id. This is the
   preferred recovery path when the leaf exists locally.
3. **`turn/start` directly**: Does not work; the app-server must have the thread
   loaded in memory first. Calling `turn/start` on an unloaded thread returns
   "thread not found".
4. **Replacement thread via `thread/start`**: Loses Codex-private history but
   keeps the Vibe workspace usable. Used as last resort.
