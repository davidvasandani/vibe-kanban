# Feature Specification: Remote-to-local workspace archive reconciliation

**Feature dir**: `specs/003-remote-local-archive-reconciliation/`
**Status**: Clarified (no open questions)
**Task**: `vk/f464-vk-workspace-mgm`
**Scope (Constitution)**: `packages/web-core` shared provider logic; no remote
mutation transaction changes.

## Summary
When a cloud issue reaches a terminal status, the remote service archives its
linked remote workspace records in the same transaction as the issue update.
The matching local workspace on the connected Vibe Kanban host can remain
active, leaving the cloud issue panel showing an archived workspace while the
local sidebar still lists that same workspace under Active. This feature
reconciles remote archived workspace state back to the linked local workspace
through the existing local workspace update path, so terminal issue archival
converges across remote and local views.

## User Stories
- As a user who marks or sees a cloud issue as Done or Cancelled, I want the
  linked local workspace to leave Active too, so the remote issue and local
  sidebar agree about completed or abandoned work.
- As a user with multiple linked workspaces, I want one failed archive attempt
  not to block other workspaces from reconciling, so unrelated workspace state
  still converges.
- As a user who explicitly unarchives a workspace, I do not want stale remote
  data to automatically revive or flip local resources except for the
  safety-oriented archive transition.

## Functional Requirements
- FR-1: While both remote project workspace data and local workspace data are
  available, the app MUST detect linked workspace records where the remote
  workspace is archived and the corresponding local workspace is still active.
- FR-2: For each detected mismatch, the app MUST archive the local workspace via
  the existing local workspace update API, preserving established local archive
  behavior such as database flag updates, process cleanup, archive scripts,
  stream updates, and local-to-remote convergence.
- FR-3: Reconciliation MUST be idempotent. The app MUST NOT repeatedly submit
  archive requests for the same local workspace while an archive request for
  that workspace is already in flight.
- FR-4: A failure archiving one local workspace MUST NOT prevent other detected
  mismatches from being archived.
- FR-5: Failed archive attempts MUST remain retryable after relevant remote or
  local workspace data changes, or after the provider remounts.
- FR-6: The app MUST NOT automatically unarchive a local workspace based on
  remote workspace state.
- FR-7: Remote-only workspaces and local workspaces without a remote link MUST
  be ignored by this reconciliation.
- FR-8: The existing remote issue mutation behavior MUST remain unchanged:
  terminal issue workspace archival stays transaction-bound and txid-covered on
  the remote server.

## Out of Scope
- Changing which issue statuses are terminal or how terminal statuses are
  detected.
- Moving, weakening, or duplicating the existing remote transaction that
  archives workspace rows when an issue enters Done or Cancelled.
- Adding a database migration, new API endpoint, or generated type change.
- Automatically unarchiving local workspaces when remote rows are active again.
- Changing local-to-remote synchronization for explicit local archive or
  unarchive actions.
- Rendering new UI controls or changing workspace card/panel presentation.

## Acceptance Criteria
- [ ] Given a remote workspace row with `archived=true` and a linked local
      workspace with `archived=false`, the local workspace update API is called
      with `{ archived: true }` for that local workspace.
- [ ] Given an already archived linked local workspace, reconciliation submits
      no local update.
- [ ] Given an active remote workspace row, reconciliation submits no local
      archive update.
- [ ] Given a remote workspace without a `local_workspace_id`, reconciliation
      ignores it.
- [ ] Given a local workspace without the matching remote link, reconciliation
      ignores it.
- [ ] Given duplicate remote rows or repeated remote/local snapshot updates
      while a local archive request is in flight, reconciliation submits at most
      one in-flight request per local workspace.
- [ ] Given one local archive update failure among multiple mismatches, other
      eligible local workspaces are still archived.
- [ ] Given a failed archive attempt, a later data change or provider remount can
      retry the local archive request.
- [ ] No automatic local unarchive request is submitted for a remote active
      workspace and local archived workspace.
- [ ] Focused unit tests cover the pure mismatch-selection helper, including
      archived matches, remote-only rows, already archived local rows, active
      remote rows, duplicate links, and multiple eligible workspaces.
- [ ] A hook/provider test or equivalent focused frontend test covers in-flight
      deduplication and failure isolation.
- [ ] Frontend type checking, relevant tests, and `pnpm run format` pass.

## Clarifications (resolved)
- **Trigger source**: Treat remote workspace snapshots as level-triggered state,
  not as a single Electric event. Reconciliation runs from the currently
  available remote workspace collection and local workspace collection, covering
  live Electric updates, REST fallback snapshots, reconnects, and provider
  remounts.
- **Reconciliation location**: Add a small reconciliation hook in
  `packages/web-core` and invoke it from the remote project provider, where the
  Electric workspace shape is already subscribed. This keeps the issue panel and
  workspace cards presentational and lets live Electric updates, REST fallback
  snapshots, reconnects, and provider remounts all converge from the current
  workspace snapshot.
- **Local archive path**: Use `workspacesApi.update(id, { archived: true })`
  rather than adding a new endpoint or directly mutating local state. This keeps
  all existing local archive lifecycle behavior centralized.
- **Comparison logic**: Keep the mismatch selection as a pure exported helper so
  edge cases can be tested without rendering providers.
- **In-flight behavior**: Track in-flight local workspace IDs. A settled request
  leaves the in-flight set so failures remain retryable on later reconciliation
  cycles.
- **Failure handling**: Dispatch archive attempts independently per eligible
  local workspace. An error for one workspace is recorded or logged by the hook
  path as appropriate, but it does not cancel or suppress attempts for other
  eligible workspaces in the same reconciliation cycle.
- **No auto-unarchive**: Archival is the safety-oriented terminal transition at
  issue scope. Unarchiving stays an explicit workspace action to avoid reviving
  stopped local resources due to stale remote data.
- **Prior knowledge applied**: Existing remote terminal-status archival is
  correct and should stay atomic with the issue mutation. The missing behavior
  is convergence after archived remote workspace rows become visible to the
  shared web provider.

## Open Questions
- None remaining.
