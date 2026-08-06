# Prior Knowledge: Codex Rollout Lineage Transfer

Task: `c8a9-transfer-codex-r`

The project knowledge base is populated. The most relevant pages are
`clustered-workspace-execution.md`, `interrupted-worktree-recovery.md`, and
`active-mcp-refresh.md`. The affinity migration feature branch also contains a
pending `workspace-affinity-migration.md` page whose durable-operation rules
are directly relevant to this work.

## Coordinator/worker authority

- The coordinator is authoritative for SQLite records, workspace placement,
  and user-visible execution state. Workers own only processes and host-local
  capabilities for work assigned to them.
- Persist worker identity on workspace placement and execution dispatch. Never
  infer affinity from UI state, a hostname in a payload, or whichever worker
  happens to answer.
- Coordinator-to-worker requests are signed over timestamp, method, full path
  and query, and the digest of the exact body bytes. Axum middleware must use
  `OriginalUri` when nested routing rewrites the visible URI.
- Apply an explicit body limit before buffering signed bodies. Any rollout
  transport must account for wire-encoding expansion as well as raw file size.
- Anti-replay and idempotent retry are complementary: refresh authority
  timestamp/nonce on retry while keeping the operation identity and request
  digest stable. Replaying the identical signed envelope remains forbidden.

## Durable affinity migration

- Live migration is one coordinator-owned durable operation, not a browser
  sequence. It claims one operation per workspace, records the source
  execution before stopping, revalidates after claim, and uses deterministic
  continuation execution identity.
- Completed retries replay the stored result before looking at current mutable
  state. Ambiguous transport failures retain operation identity; conclusive
  request errors release it so a corrected request becomes a new operation.
- An unproven stop leaves placement unchanged. A restart failure after a proven
  placement commit must be represented as a precise stopped-on-new-affinity
  state, never as success.
- A stale operation can resume only from durable evidence. Filesystem presence
  is useful evidence but is not, by itself, an operation state machine.
- Dispatch retries keep the same execution identity and request digest and must
  reject a mismatched worker or request. This is what prevents duplicate
  continuations after response loss.

## Evidence and uncertainty

- Existing paths do not prove the expected storage is mounted or the expected
  object is present. Prefer positive structural/integrity evidence; unknown is
  not success.
- Worker event recovery is monotonic and cursor-backed: duplicates are ignored,
  gaps are rejected, and completion is never invented. Session transfer should
  use the same spirit—manifest digest plus per-entry verified checksums, with no
  inference from a partially populated target directory.
- An offline/unreachable worker is indeterminate. Do not convert uncertainty
  into permission to stop, move, clean up, or overwrite state.
- Cleanup requires positive evidence that no active/recoverable operation still
  references the artifact. Re-derive the blast radius when a cleanup process is
  introduced into a shared namespace.

## Filesystem safety patterns

- Assert the structure of resolved paths, not their spelling. A lexical prefix
  or absence of a known-bad prefix is not containment proof.
- Existence proves little: validate the expected regular-file type, identity,
  checksum, and canonical containment, and reject symlinks and ambiguous
  destination components.
- Repair/reuse only from proven evidence and refuse conflicts. Never overwrite
  an existing object merely because a retry expects to own that name.
- Keep potentially destructive cleanup scoped to the exact recorded object;
  directory-wide cleanup can silently expand across other workspaces/sessions.

## Lifecycle preservation

- Process teardown and durable state preservation are independent concerns.
  A teardown failure must not skip preservation, and preservation failure must
  not be disguised by changing execution state.
- The order of lifecycle transitions is load-bearing. For this task, verified
  lineage staging must become a hard precondition of the existing source stop;
  no later recovery path can compensate for stopping before transfer proof.
- Partial success must be reported truthfully. Successful immutable file
  installs cannot be rolled back safely if other lineage entries fail, so the
  operation record and target verification status must describe exactly what
  is complete and what remains retryable.

## Codex protocol boundary

- Codex app-server capabilities are narrower than convenient product
  assumptions. The knowledge base requires unsupported/unknown states to remain
  explicit instead of inferred.
- Existing Codex lifecycle code publishes controls only after initialization,
  thread registration, and `turn/start`; this confirms that a session/thread ID
  is executor-owned state whose adoption point matters.
- Raw executor errors and output are not safe public diagnostics. Use an
  allow-listed category/message/remediation contract. The same rule applies to
  rollout parsing and transfer: no JSONL contents, prompts, authenticated URLs,
  environment values, or secrets in logs/results.

## Consequences for the spec and plan

1. Extend the durable affinity operation with a pre-stop transfer phase and
   durable manifest/verification evidence.
2. Derive the source thread and worker pair from persisted execution and
   placement records; accept no caller-selected filesystem paths.
3. Reuse signed worker authority and operation-level idempotency while bounding
   every body, file, manifest, depth, and retry.
4. Make target installation atomic, conflict-refusing, checksum-verified, and
   contained beneath the sessions root with symlink-safe traversal.
5. Treat target filesystem artifacts as immutable payloads and the coordinator
   operation record as authoritative recovery state.
6. Keep non-Codex and stopped-affinity paths unchanged and cover them with
   regression tests.
7. Add cleanup only with explicit references/ages and narrowly scoped deletion
   rules; preserve active and indeterminate state.
