# Implementation Plan: Transfer Codex Rollout Lineage

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Rust 2024 workspace using Axum, Tokio, Reqwest, SQLx/SQLite, Serde, SHA-256,
  and Ed25519 request signing.
- Codex is pinned in `crates/executors/src/executors/codex.rs` at `0.144.1`.
- Codex derives its sessions root from `CODEX_HOME` in
  `crates/executors/src/executors/codex.rs`.
- Coordinator/worker wire types live in `crates/cluster-protocol`; signed client
  calls live in `crates/services/src/services/cluster/client.rs`; worker routes
  and signature validation live in `crates/worker/src/worker_api.rs`.
- Live migration is supplied by the prerequisite workspace-affinity branch in
  `crates/server/src/routes/workspaces/affinity.rs` and its operation table.
- Workers have local Codex homes while workspaces use shared cluster storage.
- No new external service or top-level third-party dependency is required.

## Architecture & Approach

### 1. Land on the affinity-migration prerequisite

Merge the completed `vk/9a64-vk-workspace-aff` branch before feature edits.
Resolve root artifact conflicts in favor of this task while retaining that
branch's durable affinity route, DB operation, UI, tests, and knowledge. This is
necessary because the rollout phase must sit inside its coordinator-owned
operation before `stop_execution`.

### 2. Shared rollout artifact module

Add `crates/executors/src/executors/codex/rollout_transfer.rs` so the existing
Codex home resolver and both worker/coordinator callers use one convention.
The module owns:

- strict UUID thread parsing and safe `sessions/YYYY/MM/DD/rollout-...jsonl`
  relative-path parsing;
- symlink-refusing component traversal below a canonical sessions root;
- bounded first-canonical-`session_meta` parsing for `id`, `forked_from_id`,
  `parent_thread_id`, and `cwd` identity only;
- ancestor resolution with cycle/conflict/depth/count/byte caps;
- streaming SHA-256 and immutable manifest construction;
- operation-scoped temporary writes, atomic no-clobber install, private mode,
  reopen verification, same-content reuse, and conflict refusal; and
- narrowly scoped temporary/verified cleanup helpers.

Do not deserialize or retain the rest of a rollout line. Diagnostic errors are
an allow-listed enum carrying only safe IDs, relative paths, counts, sizes, and
checksums.

### 3. Wire contracts

Extend `crates/cluster-protocol/src/lib.rs` with operation-bound request and
response types described in `contracts/worker-session-transfer.md`. Each
mutating payload includes `RequestAuthority`; `correlation_id` equals the
affinity operation ID. Manifest digest and workspace/source/target/leaf IDs are
repeated and validated at every phase to prevent substitution.

Payload bytes use a chunked protocol (bounded base64 chunks in signed JSON)
rather than one 32 MiB body. Each chunk is independently bounded and ordered;
finalization validates total size and SHA-256. This fits the existing signed
body mechanism and avoids adding peer-worker trust or unsigned streaming.

### 4. Source and target worker routes

In `crates/worker/src/worker_api.rs`, add signed routes under
`/v1/session-transfers/{operation_id}` for manifest resolution, artifact chunk
read, target chunk stage, finalize, verify/status, and abort-partials.

`WorkerApiState` receives a `CodexRolloutTransferStore` built from the same
`CODEX_HOME` convention the executor uses. Route handlers validate authority,
path IDs, worker role, workspace/operation binding, manifest digest, chunk
offset, and declared entry before calling the shared module. Lower the route's
per-body cap below the generic 72 MiB cap with handler-level bounded decoding.
Raw artifact bytes never appear in error formatting or tracing fields.

### 5. Coordinator client and orchestrator

Add typed methods to
`crates/services/src/services/cluster/client.rs` with a two-minute overall
deadline and per-response caps. A new
`crates/services/src/services/cluster/session_transfer.rs` orchestrator:

1. obtains and validates the source manifest;
2. persists manifest entries and digest;
3. reads each source entry in fixed chunks and forwards it to the target;
4. finalizes each entry ancestor-first;
5. asks the target to re-verify the complete manifest; and
6. conditionally persists `verified` evidence.

Local source/target placement calls the same `CodexRolloutTransferStore`
directly. The orchestrator never writes payloads to disk or logs them.

### 6. Durable operation state and lifecycle gate

Add a SQLx migration and models described in `data-model.md`. Extend
`workspace_affinity_operations` with a phase and transfer-safe outcome fields;
use normalized transfer/artifact rows for immutable evidence and cleanup.

In `crates/server/src/routes/workspaces/affinity.rs`, after operation claim and
Codex/source/target derivation but before source stop:

- create or replay the transfer record;
- run/resume the orchestrator;
- require a matching durable `verified` manifest digest;
- touch/revalidate the affinity operation; then
- enter the existing stop → placement → deterministic continuation path.

Map failures before verification to a new `SessionTransferFailed` response
outcome with safe category/phase and leave execution/placement untouched.
Existing post-stop partial outcomes remain unchanged. Re-verify immediately
before continuation dispatch after crash recovery.

### 7. Cleanup and deployment

Add bounded startup and periodic cleanup owned by the deployment/container
service. It queries DB candidates first, protects active/recoverable references,
then calls the shared cleanup helper on the owning node. Partials older than 24
hours and verified artifacts unused for 30 days are candidates. File type and
containment are rechecked immediately before deletion.

Prefer constants for limits/retention in this first implementation. Change
`homelab/modules/vibe-kanban-rebuild.nix` only if the worker service currently
does not give the transfer store the exact same `CODEX_HOME`, user/group, or
private-directory semantics as Codex execution; no other module is in scope.

### 8. Verification

- Unit-test parser/path/hash/stager/cleanup helpers with temp directories,
  symlinks, mutation during read, cycles, ancestor chains, size boundaries,
  identical retries, and conflicts.
- Route-test signature, authority correlation, worker/source/target/workspace
  substitution, chunk ordering, body caps, and safe error serialization.
- SQL/state-machine-test all crash windows and conditional transitions.
- Extend affinity route tests for pre-stop failure preservation, verified gate,
  exact-once continuation, non-Codex, same-worker, and stopped paths.
- Add a two-worker fixture with source/target temp Codex homes and invoke the
  pinned app-server `thread/fork` contract against the target when feasible;
  otherwise use a pinned-format compatibility fixture plus an opt-in executable
  integration test documented in validation.
- Run generated types, format, focused Rust tests, backend check/clippy, and
  relevant Nix evaluation.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/worker-session-transfer.md` and
`./contracts/affinity-session-transfer.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- I–III, VI: extends the existing migration, worker client, signer, and Codex
  resolver in bounded reversible layers with contract tests.
- IX: reads only pinned Codex 0.144.1 canonical metadata and rejects unknown or
  inconsistent identity rather than inferring it.
- XII, XVIII: one durable coordinator owner, conditional phases, explicit
  worker affinity, deterministic retries, and positive worker evidence.
- XV: no source stop or placement mutation precedes verified transfer; unknown
  and cleanup errors retain state.
- XVII, XXI: target confirmation is required and failures name a safe specific
  transfer category.
- XX: every path is derived and structurally contained on the node that uses it.
- XXII: manifest-only transfer, no executor-home copy, content conflicts refuse
  overwrite, and cleanup is bounded/age-based.

No constitution deviations remain.

## Risks & Dependencies

- The task branch lacks the affinity-migration prerequisite; merge conflicts
  are likely because both tasks own root planning artifacts.
- Codex rollout format can change. Pin fixtures to 0.144.1, isolate parsing,
  require canonical IDs, and fail closed on incompatible metadata.
- Codex's SQLite index is mutable and intentionally not copied. Version 0.144.1
  can discover rollout files when the DB lacks a row; the end-to-end test must
  prove this remains true for the pinned version.
- A rollout may still be changing while its source task runs. Resolve/hash from
  an open regular-file handle, check metadata before/after streaming, and reject
  mutation; do not stop the source to manufacture immutability before proof.
- Chunked signed JSON adds round trips but keeps memory/body bounds explicit.
  Limits and two-minute deadline prevent an operation from hanging indefinitely.
