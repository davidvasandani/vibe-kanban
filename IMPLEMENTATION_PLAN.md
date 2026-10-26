# Implementation plan — guarded remote auto-archive

1. Add Workspace::try_archive_automatically in crates/db/src/models/workspace.rs using one conditional UPDATE over current pinned, execution and wake-up delivery state. Return whether this request transitioned the row. Reuse the running-poller predicate semantics.
2. Split cleanup from the unconditional archived write in crates/services/src/services/container.rs. Manual archive writes as before then cleans up; automatic archive cleans up only after guarded transition.
3. Add POST /api/workspaces/{id}/archive/automatic in crates/server/src/routes/workspaces/core.rs and register in mod.rs. Read the current remote record, ignore stale remote=false, then invoke guarded archive. Return archived/blocked/not_requested/already_archived outcome and surface failures.
4. Add workspacesApi.archiveAutomatically in packages/web-core/src/shared/lib/api.ts. Switch shared/providers/remote/useRemoteLocalArchiveReconciliation.ts to this dedicated path, with a 30-second mounted retry and existing in-flight deduplication. Keep manual update callers unchanged.
5. Test guard conditions against migrated SQLite, cleanup admission, current-remote handling, frontend transport and provider regressions. Install dependencies, format, run focused tests and applicable type/lint/build checks.
6. Run independent Codex CLI review; fix findings and repeat verification. Document reusable archive and continuation semantics in the project KB and commit both repos.
7. Open PRs through VK Create PR, wait for passing CI, then merge. No deployment changes are needed in homelab.

Risks: remote intent can change between its read and local mutation (separate stores); local state guard is atomic. Remote failures fail closed. UI-mounted retry preserves current convergence boundary and is not a headless worker. Clone must be associated with VK for its Create PR flow before delivery.
