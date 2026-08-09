# Clarifications: Transfer Codex Rollout Lineage

`/speckit.clarify` resolved all blocking questions using the task safety
boundaries, project constitution, observed fleet artifact sizes, existing
coordinator/worker protocol, and the source of pinned Codex CLI `0.144.1`
(`rust-v0.144.1`).

## Decisions

1. **Canonical rollout metadata is the first `session_meta` item.** Its
   `payload.id` is the rollout's thread identity. Codex 0.144.1 explicitly
   treats the first `SessionMeta` as canonical because forked rollouts retain
   copied source metadata later in the file. `forked_from_id` records explicit
   fork ancestry and `parent_thread_id` records spawned-agent ancestry. A file
   is rejected when the canonical ID does not match the requested/filename ID.

2. **Transfer the complete referenced ancestry, even though the pinned
   `thread/fork` implementation reads the leaf rollout directly.** In 0.144.1,
   `thread/fork` resolves the requested rollout, reads its complete copied
   history, and creates the new fork from that history; the leaf is sufficient
   for the immediate call. However, the feature contract requires lineage
   portability and ancestor coverage. Follow `parent_thread_id` first (spawn
   lineage) and `forked_from_id` second (fork provenance), rejecting cycles and
   conflicts, and install ancestors before the leaf. This is conservative,
   supports later lineage reads, and directly satisfies the acceptance test.

3. **Bytes are coordinator-mediated.** The coordinator already has separate
   authenticated authority to both workers; workers do not have peer
   credentials or an authorization model for calling one another. A direct
   worker transfer would add a second trust topology. The coordinator streams
   bounded chunks from an operation/manifest-authorized source read into an
   operation/manifest-authorized target stage without persisting contents.

4. **Hard limits are 32 MiB per file, 128 MiB total, 32 artifacts/depth, and
   two minutes per transfer attempt.** Observed rollout files on the reproduced
   fleet are below 2 MiB, leaving substantial headroom. Limits are server-owned
   constants/config with lower test overrides, checked both before and during
   streaming so a growing or substituted source cannot bypass the manifest.

5. **Verified artifacts are retained for 30 days; partial temporary files for
   24 hours.** Active/recoverable operations and executions override age and
   remain protected. Successful continuation use refreshes the staged
   artifact's last-needed timestamp. Cleanup runs at startup and periodically,
   is bounded per pass, and only considers Vibe-Kanban-recorded staged paths.

6. **Use normalized durable transfer tables linked to the affinity operation.**
   One transfer row holds operation/workspace/source/target/leaf/manifest/phase
   and safe failure evidence. Child artifact rows hold ordered thread IDs,
   parent relationship, safe relative path, size, checksum, target verification
   time, and last-needed time. This supports conditional transitions,
   idempotent recovery, exact cleanup references, and conflict diagnosis
   without storing rollout contents.

7. **Session-transfer failure is an affinity-operation outcome, not a raw Codex
   error.** Before target verification, all such failures preserve source
   execution and placement. The response identifies an allow-listed category
   and phase plus safe IDs/checksum/size/path facts. Contents and executor raw
   output never cross the diagnostic boundary.

8. **Coordinator-local endpoints reuse the same library and evidence model.**
   Local source/target operations call the contained resolver/stager directly;
   remote endpoints wrap those same operations with signed worker authority.
   Local placement is not a reason to weaken validation or omit verification.

## Remaining Questions

None.
