# Feature Specification: Transfer Codex Rollout Lineage

**Feature dir**: `specs/vk/c8a9-transfer-codex-r/`
**Status**: Draft

## Summary

Preserve Codex continuation history when a running workspace changes execution
servers. Before Vibe Kanban stops the source task or commits the new affinity,
it must transfer and verify only the immutable rollout artifacts required to
continue the prior Codex thread. Unsafe or incomplete transfers leave the
running task and placement unchanged and produce a specific, actionable session
transfer result.

## User Stories

- As an operator, I want a running Codex workspace to continue normally after I
  migrate it to another worker so that changing execution capacity does not
  strand the task.
- As an operator, I want a failed session transfer to preserve the running
  source task and its affinity so that migration is fail-safe.
- As an operator, I want transfer failures to identify the failed phase and
  safe reason so that I can remediate missing, corrupt, oversized, conflicting,
  or unauthorized artifacts without inspecting secret rollout contents.
- As a platform maintainer, I want retries and crash recovery to reuse verified
  rollout artifacts and create at most one continuation so that an ambiguous
  response cannot duplicate or corrupt work.
- As a security owner, I want only the required immutable rollout lineage moved
  between authorized workers so that credentials, mutable databases,
  configuration, caches, and logs in the executor home remain isolated.

## Functional Requirements

- **FR-1:** The system MUST determine from authoritative workspace, execution,
  session, executor, and placement records whether a live affinity migration
  requires a Codex session transfer.
- **FR-2:** A transfer MUST occur only for a running Codex migration whose
  managed continuation will use a prior Codex thread on a different execution
  server. Non-Codex migrations, same-server changes, and stopped affinity
  changes MUST retain their existing behavior.
- **FR-3:** The source MUST resolve the requested Codex rollout and every
  ancestor artifact required to continue that thread, and MUST prove that the
  resolved lineage represents the authorized thread and migration context.
- **FR-4:** The source MUST reject missing, unreadable, malformed, conflicting,
  cyclic, duplicate, or over-depth lineage and artifacts that exceed the
  allowed file count, individual size, or total size.
- **FR-5:** Source discovery MUST be restricted to regular rollout artifacts
  inside the configured Codex sessions area and MUST reject absolute caller
  paths, traversal, symlinks, and destination/source escape.
- **FR-6:** The transfer description MUST identify the authorized operation,
  workspace, source, target, leaf thread, ordered lineage entries, sizes,
  checksums, and one digest that uniquely represents the complete manifest.
- **FR-7:** Transfer MUST use existing coordinator/worker authentication,
  authorization, affinity, request-integrity, and replay protections. A request
  MUST NOT be reusable for another operation, workspace, worker pair, thread,
  or manifest.
- **FR-8:** The system MUST enforce explicit bounds for each artifact, the
  complete lineage, file count, lineage depth, and transfer duration, without
  retaining unbounded payloads in memory.
- **FR-9:** The target MUST stage each artifact privately, verify its declared
  identity, size, and checksum, install it atomically at the safe Codex-visible
  destination, and confirm its ownership, permissions, file type, containment,
  and readability after installation.
- **FR-10:** The target MUST acknowledge the transfer as complete only when the
  entire ordered manifest is installed and verified. A path's existence alone
  MUST NOT count as completion evidence.
- **FR-11:** Repeating an identical operation and manifest MUST reuse already
  verified identical artifacts without duplicating or corrupting them.
- **FR-12:** If an artifact for the same thread identity already exists with
  different content, the target MUST reject the transfer and MUST NOT overwrite
  or hide the conflict.
- **FR-13:** The coordinator MUST durably record the authorized transfer and
  target verification evidence before it may stop the source execution or
  change workspace affinity.
- **FR-14:** A failure before complete target verification MUST leave the source
  execution running and workspace affinity unchanged.
- **FR-15:** Recovery after a crash or ambiguous response MUST resume from
  durable operation and verification evidence. It MUST NOT infer transfer
  success from current placement or filesystem presence alone.
- **FR-16:** Once transfer is verified, existing migration behavior MUST retain
  its deterministic continuation identity so duplicate requests cannot create
  duplicate continuation executions.
- **FR-17:** If target verification can no longer be proven before continuation
  dispatch, the system MUST re-verify or stop with a specific transfer failure;
  it MUST NOT start the managed continuation speculatively.
- **FR-18:** Failed attempts MUST remove their operation-scoped partial files
  when safe while retaining durable, content-free evidence of the failed phase
  and category.
- **FR-19:** Verified transferred artifacts MUST be retained for an explicit
  age sufficient for retries and crash recovery. Cleanup MUST protect artifacts
  referenced by active or recoverable operations and executions.
- **FR-20:** Cleanup MUST affect only artifacts known to have been staged by
  Vibe Kanban, revalidate type and containment before removal, never follow
  symlinks, and be safe to repeat.
- **FR-21:** Public results and logs MUST use safe, specific session-transfer
  categories such as missing lineage, corrupt lineage, size limit,
  authorization failure, checksum mismatch, target conflict, or verification
  failure.
- **FR-22:** Rollout contents, prompts, tokens, credentials, environment values,
  authenticated URLs, mutable Codex state, and other secrets MUST never appear
  in logs, operation evidence, public results, or metrics.
- **FR-23:** The system MUST NOT copy or synchronize the complete Codex home or
  transfer mutable databases, credentials, configuration, caches, or logs.
- **FR-24:** Transfer behavior MUST work when either endpoint is the coordinator
  host as well as when both endpoints are workers, without weakening the same
  authorization, validation, idempotency, and evidence requirements.

## Out of Scope

- General-purpose file transfer between execution servers.
- Replicating a complete executor home or making Codex state globally shared.
- Moving or changing services other than Vibe Kanban.
- Redesigning worker scheduling, worker draining, or affinity selection.
- Automatically evacuating workspaces when a worker becomes unhealthy.
- Changing Codex's own rollout format or `thread/fork` behavior.
- Transferring non-Codex executor state in this feature.

## Acceptance Criteria

- [ ] Migrating a running Codex workspace between two workers transfers the
      required rollout lineage and the managed continuation successfully forks
      the prior thread on the target.
- [ ] A lineage with direct and multi-level ancestor rollouts is fully present,
      checksum-verified, private, correctly owned, and readable on the target
      before any source stop or placement mutation.
- [ ] Missing, malformed, corrupt, oversized, unauthorized, traversal-based,
      symlinked, or conflicting source/target artifacts fail with a specific
      session-transfer result and leave the original execution and affinity
      unchanged.
- [ ] Retrying the same migration reuses matching artifacts, refuses conflicting
      content, and creates exactly one continuation execution.
- [ ] Recovery tests cover crashes before transfer, during staging, after
      target verification, after source stop, and after placement commit.
- [ ] Failed staging removes partial temporary payloads without deleting
      verified artifacts or exposing their contents.
- [ ] Age-based cleanup removes eligible expired staged artifacts while
      retaining active/recoverable lineage and unrelated Codex session files.
- [ ] Authorization tests reject operation, workspace, thread, source-worker,
      and target-worker substitution.
- [ ] Non-Codex live migrations and stopped affinity changes pass regression
      tests with their existing behavior unchanged.
- [ ] An end-to-end two-worker test demonstrates that target-side `thread/fork`
      can resolve the transferred lineage rather than returning a generic
      rollout-not-found I/O error.

## Open Questions

Resolved in `clarifications.md`. No questions remain open.
