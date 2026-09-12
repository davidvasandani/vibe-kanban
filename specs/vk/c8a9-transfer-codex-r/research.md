# Research: Transfer Codex Rollout Lineage

## Pinned Codex 0.144.1 behavior

The repository pins `@openai/codex@0.144.1`. Inspection of upstream tag
`rust-v0.144.1` established:

- `thread/fork` calls the local thread store with `include_history=true`,
  resolves the requested rollout, reads its history, and builds a new thread
  from the copied history.
- `SessionMeta` contains `id`, `forked_from_id`, and `parent_thread_id`.
- Forked rollout files contain the new canonical `SessionMeta` first and copied
  source `SessionMeta` items later; Codex explicitly uses the first item as the
  canonical thread contract.
- The leaf rollout is sufficient for the immediate fork because fork history is
  copied. The complete referenced ancestry is still transferred to meet the
  product lineage guarantee and preserve future lineage operations.
- The local store falls back to resolving rollout files when SQLite lacks the
  thread row, but rejects missing/mismatched rollout paths. This must be guarded
  by an executable compatibility test because SQLite is intentionally excluded.

## Transfer topology

Chosen: coordinator-mediated fixed-size chunks over existing signed worker
requests. Rejected:

- worker-to-worker: requires new peer credentials, discovery, firewall, and
  authorization semantics;
- shared NFS: rollout homes are intentionally local and complete-home sharing
  violates the task boundary;
- one base64 body: raises memory and signed-body caps for the worst-case file;
- rsync/scp: path-general and not bound to workspace/thread manifest semantics.

## Integrity and installation

Use SHA-256 already present in worker/services/executors dependencies. No new
cryptographic dependency. Hash an open regular-file handle, validate size and
identity around streaming, write target chunks to an operation-scoped partial,
sync/close, reopen/hash, and install without overwriting existing content.

Linux `renameat2(RENAME_NOREPLACE)` would be ideal but adds platform-specific
surface. The implementation may use a private operation staging directory plus
hard-link/no-clobber or `create_new` destination semantics; the tasks require a
race test before selecting the exact primitive.

## Limits and retention

Observed fleet rollouts are below 2 MiB. Selected limits: 32 MiB/file,
128 MiB/lineage, 32 files/depth, two minutes/attempt. Verified artifacts retain
30 days since last-needed; partials retain 24 hours. These are intentionally
generous, bounded defaults and do not require deployment configuration initially.

## Dependencies

No new top-level dependency is planned. Use existing `sha2`, `tokio`, `serde`,
`serde_json`, `uuid`, `chrono`, `reqwest`, `axum`, and `sqlx` dependencies.
