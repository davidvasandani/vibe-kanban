# Implementation Plan: Issue and workspace lifecycle

**Spec**: `./spec.md`

## Architecture & Approach
- `crates/services/src/services/container.rs`: central activation hook used by execution startup, preserving ArchiveScript exclusion.
- `crates/services/src/services/remote_sync.rs`: persist activation and schedule existing best-effort remote sync, with a mock HTTP regression.
- `crates/local-deployment/src/container.rs`: implement activation using that helper; avoid sending stale archive state at completion.
- `crates/server/src/routes/sessions/queue.rs`: activate at queue acceptance so messages do not wait for execution completion.
- `crates/remote/src/db/workspaces.rs`: extend existing update with one transaction, locked prior state and conditional Done-to-In progress issue update. Lock issue before workspace to match terminal-issue archival lock order. Preserve existing workspace response contract (this legacy endpoint returns Workspace, not MutationResponse).
- Reuse existing project status resolution and remote sync; no dependency, schema or generated API changes.

## Verification
Add focused regression tests of persisted transitions and activation behavior. Run locked install and repository formatting; run backend checks and focused tests. Independent Codex review must evaluate synchronization, concurrency and unintended status changes.

## Constitution Check
II covered by regressions; III/VI reuse existing paths; V all remote side effects in one transaction; XII queue handling remains owned by existing service; XXI established status names. The existing workspace endpoint has no txid envelope; do not change its public contract here.

## Risks
Local/remote synchronization is best effort, with post-login reconciliation, not a distributed transaction. Concurrent archive/unarchive must use current persisted state. Missing named target status preserves issue state. Existing initial execution unarchives even before dispatch succeeds; preserve that established boundary.
