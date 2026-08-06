# Technical Specification: Codex Rollout Lineage Transfer

**Task:** `vk/c8a9-transfer-codex-r`  
**Scope:** Vibe Kanban service and its governing
`homelab/modules/vibe-kanban-rebuild.nix` deployment only  
**Status:** Draft for SpecKit refinement

## Objective

Allow a running Codex workspace to move between execution workers without
breaking the managed continuation. Before the coordinator stops the source
execution or changes persisted placement, it must copy the exact immutable
Codex rollout lineage needed by `thread/fork` from the source worker's local
`CODEX_HOME/sessions` tree into the target worker's corresponding sessions
tree and obtain checksum-backed proof that the complete lineage is readable.

## Background

Workspace affinity migration is a coordinator-owned durable operation. It
records the running execution, stops it, changes placement, and creates one
idempotent managed follow-up. For Codex, that follow-up resumes the recorded
session through the app-server `thread/fork` request.

Codex rollout JSONL files are deliberately worker-local. A target worker can
therefore have the shared workspace and conversation metadata while lacking
the rollout file named by the prior Codex thread ID. The resulting
`no rollout found for thread id ...` error is a missing immutable session
artifact, not a workspace-storage or disk failure.

## Scope

- Detect whether a live affinity migration's source execution uses Codex and
  has a resumable Codex thread ID.
- Resolve the source rollout plus every ancestor rollout required to fork the
  thread, based on validated metadata in the JSONL artifacts.
- Add authenticated coordinator-to-worker protocol operations to describe,
  read, stage, and verify a bounded rollout lineage.
- Transfer artifacts directly under coordinator control before any source stop
  or placement mutation.
- Install only immutable rollout JSONL files beneath the target worker's Codex
  sessions directory, preserving the relative layout Codex expects.
- Persist enough transfer state/evidence with the affinity operation to make
  retries and crash recovery deterministic.
- Add age-based cleanup for transferred artifacts that are no longer needed.
- Wire the required path, limits, retention, and ownership through the Vibe
  Kanban deployment module if runtime defaults cannot safely derive them.

## Out of Scope

- Copying or synchronizing all of `CODEX_HOME`.
- Copying Codex SQLite databases, credentials, configuration, logs, caches, or
  other mutable state.
- Changing migration behavior for non-Codex executors.
- Changing stopped-workspace affinity updates, which do not create a managed
  continuation.
- Changing any service other than Vibe Kanban.
- General-purpose worker file transfer or arbitrary path access.

## Functional Requirements

### 1. Determine when transfer is required

The coordinator must derive executor kind, source thread ID, workspace ID,
source worker, and selected target worker from durable server-side records. It
must not trust client-supplied paths, thread IDs, worker identities, or
workspace substitutions.

The transfer phase runs only when all of these are true:

- the affinity change is a live migration with a managed continuation;
- the source execution is a Codex initial request or Codex follow-up;
- the continuation has a persisted Codex thread/session ID;
- source and target are distinct workers.

All other affinity changes retain their current behavior.

### 2. Resolve a complete lineage on the source worker

The source worker must locate rollout files only inside its configured
`CODEX_HOME/sessions` root. It must parse the bounded JSONL stream without
logging its contents, validate that the requested thread ID is represented by
the artifact, discover any parent thread ID required by the fork lineage, and
repeat until reaching a root.

Resolution must reject:

- missing or unreadable artifacts;
- malformed or conflicting thread metadata;
- cycles, duplicate thread substitution, or a lineage exceeding the maximum
  depth;
- a path outside the sessions root, including traversal and symlink escape;
- non-regular files;
- an artifact or total lineage exceeding configured byte limits; and
- workspace/session metadata that does not match the coordinator-authorized
  migration context where that metadata is available in the rollout.

The source returns a manifest containing only validated identifiers, safe
relative destinations, byte sizes, and SHA-256 checksums. Rollout contents and
secrets must never appear in logs, API errors, operation results, or metrics.

### 3. Stage and verify on the target worker

The coordinator transfers exactly the manifest entries over the existing
authenticated coordinator/worker authority. The worker protocol must bind the
request to the operation, workspace, source worker, target worker, and thread
lineage and must retain the existing signed-request replay protections.

For each artifact the target must:

1. resolve a safe destination beneath its configured sessions root;
2. reject pre-existing symlinks or non-regular destination components;
3. write to an operation-scoped temporary file with bounded permissions;
4. stream-hash and size-check the payload;
5. set the service ownership and final read-only/private permissions;
6. atomically install the artifact; and
7. reopen and verify the installed file before acknowledging it.

The target must acknowledge the complete lineage only after every manifest
entry is installed and readable. A target-side manifest/status read gives the
coordinator durable verification evidence without returning rollout contents.

### 4. Idempotency and conflicts

Repeating the same affinity operation and manifest must be safe:

- an already-installed artifact with the same thread ID, size, and checksum is
  reused without rewriting it;
- a pre-existing artifact for the same thread ID with different content is a
  hard conflict and is never overwritten;
- repeated completion requests return the prior verified outcome; and
- an operation cannot be replayed for a different workspace, worker pair,
  requested thread, or manifest digest.

Temporary partial files are operation-scoped and may be removed after a
failed attempt. Verified immutable artifacts are retained for retry/recovery.

### 5. Affinity lifecycle ordering and recovery

The affinity operation state machine must introduce a pre-stop transfer phase.
The coordinator may call the existing source-stop behavior only after durable
evidence records that the target verified the complete lineage.

- Failure before or during transfer leaves the running source task and
  placement unchanged.
- A crash after target verification but before stop resumes from verification
  evidence without duplicating the transfer.
- A crash after stop or placement commit uses the existing deterministic
  continuation identity and may re-verify staged lineage before dispatch.
- If verification evidence cannot be reconstructed, the coordinator must not
  infer success from placement or file existence alone.
- Duplicate migration requests must not create duplicate continuation
  executions.

### 6. Errors and observability

Transfer failures must surface as a specific session-transfer outcome that
identifies the safe category and phase (for example missing lineage, corrupt
artifact, size limit, authorization, checksum mismatch, target conflict, or
verification failure). Error messages may include thread IDs, operation IDs,
worker IDs, safe relative paths, byte counts, and checksums; they must not
include rollout content, prompts, tokens, environment values, or credentials.

Affinity-operation audit/debug logs should record phase transitions and safe
manifest facts. A failed transfer must preserve enough durable evidence for an
operator to act while deleting partial payload files.

### 7. Retention and cleanup

Verified transferred artifacts must survive long enough for continuation
retry and crash recovery. The implementation must define an explicit default
retention age and a periodic or startup cleanup pass that:

- considers only artifacts known to have been staged by Vibe Kanban;
- does not delete artifacts referenced by an active/recoverable migration or
  execution;
- uses recorded verification/last-use time rather than filename timestamps;
- revalidates containment and file type before deletion; and
- safely removes abandoned temporary files separately from verified files.

Cleanup must be idempotent and its failures non-destructive.

## Security Requirements

- Reuse existing coordinator/worker authentication, request signing,
  authorization, nonce/replay defense, and worker-affinity checks.
- No API accepts an absolute filesystem path from a client or coordinator.
- Canonical containment checks are required at both source and target; lexical
  prefix checks alone are insufficient.
- Never follow symlinks while discovering, staging, verifying, or cleaning.
- Enforce per-file, lineage-total, file-count, depth, and transfer-time limits.
- Stream bounded payloads; do not buffer an unbounded JSONL artifact in memory.
- Use constant-format errors and structured safe metadata; never log contents.
- Preserve service-user ownership and private permissions expected by Codex.

## Data and Protocol Shape

Exact names will be finalized by SpecKit, but the design needs these concepts:

- `CodexRolloutManifest`: authorized context, requested thread ID, ordered
  ancestor-to-leaf entries, and a canonical manifest digest.
- `CodexRolloutEntry`: thread ID, safe relative path, size, checksum, and parent
  thread ID when present.
- `SessionTransferState`: operation-bound phase, manifest digest, verified
  target evidence, timestamps, failure category, and safe diagnostic detail.
- Source-worker endpoints for manifest resolution and bounded artifact reads.
- Target-worker endpoints for stage/finalize/status, all under the existing
  signed coordinator authority.

The operation record is authoritative for recovery. The filesystem alone is
not an operation-state database.

## Test Requirements

Focused unit and integration tests must cover:

- direct rollout transfer and multi-level ancestor transfer;
- missing, malformed, corrupt, cyclic, duplicate, and oversized lineage;
- checksum and size mismatch during transfer;
- traversal, absolute path, symlink component, symlink file, and destination
  escape rejection;
- ownership/permission and post-install readability verification;
- same-content idempotent retry and conflicting-content rejection;
- temporary-file cleanup and age-based verified-artifact retention cleanup;
- authorization and cross-workspace/thread/worker substitution rejection;
- crashes/failures before transfer, during staging, after verification, after
  source stop, and after placement commit;
- exactly-once continuation behavior across retries; and
- an end-to-end two-worker Codex migration that successfully reaches
  `thread/fork` on the target.

Regression tests must prove non-Codex migrations and stopped affinity changes
retain current behavior.

## Acceptance Criteria

1. A running Codex workspace can migrate between two workers and its managed
   continuation successfully forks the prior thread on the target.
2. Every required rollout and ancestor is present, checksum-verified, private,
   and readable on the target before the source task is stopped or placement
   changes.
3. Missing, corrupt, oversized, conflicting, or unauthorized artifacts leave
   the original execution and affinity unchanged.
4. Repeating a migration does not corrupt artifacts or duplicate a
   continuation execution.
5. Crash recovery is deterministic at each transfer/migration boundary.
6. Failures report a specific session-transfer category instead of falling
   through to Codex's generic rollout-not-found I/O failure.
7. Cleanup removes expired staged state without endangering active/recoverable
   continuations or unrelated Codex sessions.
8. Required direct, ancestor, retry, integrity, path-safety, cleanup, and
   two-worker migration tests pass.

## Open Questions for SpecKit Clarification

- Which rollout JSONL event(s) are authoritative for thread and parent-thread
  identity in the pinned Codex CLI version?
- Does `thread/fork` require every ancestor file or only the requested rollout,
  and how will a compatibility test guard that assumption across Codex bumps?
- Should artifact bytes travel coordinator-mediated or through a narrowly
  authorized worker-to-worker channel, given current protocol capabilities?
- What per-file/total/depth limits and retention ages fit observed fleet data?
- Where should durable transfer evidence live: the affinity-operation row, a
  normalized child table, or both?
- How should coordinator-local source/target placement participate without
  weakening the same validation and idempotency rules?
