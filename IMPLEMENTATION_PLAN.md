# Implementation Plan: Codex Rollout Lineage Transfer

Task: `c8a9-transfer-codex-r`

This is the pre-SpecKit implementation plan required by the task pipeline. The
SpecKit plan and task list will refine exact schemas, contracts, file names, and
test commands after constitution/spec clarification.

1. **Bring the affinity migration prerequisite into the working branch.**
   Reconcile the branch with the current Vibe Kanban mainline that contains the
   coordinator-owned workspace affinity operation. Confirm the durable stop,
   placement, and deterministic continuation boundaries before editing them.

2. **Confirm the pinned Codex rollout contract.** Inspect the pinned Codex CLI,
   representative sanitized rollout fixtures, and current Codex executor
   session extraction to identify authoritative thread/parent metadata and the
   exact files `thread/fork` needs. Record compatibility assumptions and choose
   conservative file-count, depth, per-file, total-byte, and retention limits.

3. **Model durable transfer state.** Add database migration/model support for
   an affinity-operation-bound transfer phase, canonical manifest digest,
   source/target identities, requested leaf thread, verified timestamp, safe
   failure category/detail, and per-artifact evidence where needed. Make state
   transitions conditional and replayable.

4. **Add shared rollout-transfer types and safe primitives.** Define protocol
   manifests/status/errors and implement thread-ID validation, canonical
   manifest hashing, bounded JSONL metadata parsing, symlink-safe contained
   path traversal, streaming SHA-256, atomic private-file install, and
   same-content/conflicting-content decisions in the narrowest reusable crate.

5. **Implement source-worker lineage resolution/read.** Add signed,
   coordinator-authorized endpoints that derive paths from configured
   `CODEX_HOME`, resolve the requested leaf plus ancestors, enforce all limits
   and context bindings, and stream only a manifest-authorized artifact. Never
   log or return JSONL content as diagnostics.

6. **Implement target-worker staging/finalization/status.** Add signed,
   operation-bound endpoints that create operation-scoped temporary files,
   validate size/checksum/containment and destination conflicts, atomically
   install with service ownership/private permissions, reopen-verify the full
   lineage, and return durable safe evidence. Repeated identical requests reuse
   verified artifacts; conflicting bytes are refused.

7. **Orchestrate coordinator-mediated transfer.** Extend the worker client to
   fetch each manifest entry from the authorized source and stage it on the
   selected target with bounded streaming/backpressure. Persist progress and
   verified completion so response loss/crashes resume deterministically.

8. **Gate affinity lifecycle changes.** Insert the transfer phase after the
   affinity operation is claimed/revalidated and before source stop. Only the
   verified state may advance to stop, placement commit, and deterministic
   continuation dispatch. Map failures to explicit session-transfer outcomes
   while leaving source execution and affinity unchanged.

9. **Handle local/coordinator placement deliberately.** Route coordinator-local
   source or target through the same validation/staging implementation without
   inventing unsigned arbitrary filesystem APIs. Preserve current behavior for
   non-Codex executors, same-worker moves, and stopped affinity changes.

10. **Add retention cleanup.** Record verified/last-needed ages, remove only
    abandoned operation temporary files or expired Vibe-Kanban-staged immutable
    artifacts, protect active/recoverable references, and revalidate type and
    containment immediately before deletion. Wire startup/periodic execution
    and deployment configuration only where necessary.

11. **Test in layers.** Add pure path/parser/manifest/hash tests; worker route
    authorization, substitution, size, checksum, traversal, symlink,
    idempotency, conflict, permissions, and cleanup tests; coordinator crash
    window/state-machine tests; and an end-to-end two-worker Codex fork
    migration fixture. Retain regression coverage for non-Codex and stopped
    affinity changes.

12. **Generate, format, and verify.** Regenerate Rust-derived TypeScript types
    if public API shapes change, run formatting, focused tests, workspace checks
    in proportion to the touched crates, and Nix evaluation/tests for any
    `vibe-kanban-rebuild.nix` change.

13. **Independent review and knowledge capture.** Run the required independent
    Codex diff review, fix confirmed findings and repeat until clean. Then add
    the reusable rollout-transfer and migration-ordering lessons to the project
    knowledge base, refresh its index, and commit the knowledge-base update.
