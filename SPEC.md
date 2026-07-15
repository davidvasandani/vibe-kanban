# Technical Specification: Remote-to-Local Workspace Archive Reconciliation

## Problem

When a cloud issue enters a terminal status, the remote service archives its
linked remote workspace records in the same transaction as the issue update.
The corresponding workspace records on the connected local Vibe Kanban host
remain active. The cloud issue panel therefore shows an archived workspace
while the local workspace sidebar still shows the same workspace under Active.

Local-to-remote archive updates already synchronize through the local workspace
API and `remote_sync`; the missing direction is remote-to-local reconciliation.

## Goal

Keep a linked local workspace's archive state consistent with its remote
workspace record when remote workspace data marks it archived, including the
automatic archive caused by moving an issue to Done or Cancelled.

## Requirements

1. While remote project workspace data and local workspace data are available,
   detect linked records where the remote workspace is archived but the local
   workspace is still active.
2. Archive each matching local workspace through the existing local workspace
   update API so normal archive behavior runs (database flag, process cleanup,
   archive script, stream updates, and harmless local-to-remote convergence).
3. Reconciliation must be idempotent and must not repeatedly submit an archive
   request while the same workspace request is in flight.
4. A failure archiving one local workspace must not prevent other mismatched
   workspaces from reconciling, and failures must remain retryable after the
   remote/local data changes or the provider remounts.
5. Do not automatically unarchive a local workspace from remote state. Archival
   is the safety-oriented terminal transition at issue scope; unarchiving stays
   an explicit workspace action and avoids reviving stopped local resources due
   to stale remote data.
6. Remote-only workspaces and local workspaces without a remote link are ignored.

## Proposed Design

Add a small reconciliation hook in the shared web core and invoke it from the
remote project provider, where the Electric workspace shape is already
subscribed. The hook compares project remote workspace records with active local
workspace summaries from `WorkspaceContext`, selects archived remote records by
their `local_workspace_id`, and calls `workspacesApi.update(id, { archived:
true })` for mismatches. An in-flight ID set prevents duplicate calls across
Electric/render updates; settled calls leave the set so a later reconciliation
cycle can retry failures.

Keep the comparison logic as a pure exported helper so edge cases can be unit
tested without rendering providers. Tests cover matches, remote-only records,
already-archived local records, active remote records, duplicate links, and
multiple eligible workspaces.

## Compatibility and Scope

- No database migration or API contract change is required.
- Existing local-to-remote synchronization remains authoritative for explicit
  local archive/unarchive actions.
- The change belongs in `packages/web-core`; both local and remote frontends use
  the shared provider path.
- This task does not change which issue statuses are terminal or the remote
  transaction that archives workspace rows.

## Verification

- Run focused tests for the reconciliation helper/hook.
- Run frontend type checking and formatting for touched files.
- Exercise the regression path conceptually or manually: move a linked issue to
  Done, observe the remote record become archived, then observe the linked local
  workspace leave Active and enter Archive.
