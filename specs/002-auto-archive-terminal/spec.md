# Feature Specification: Auto-archive workspaces when an issue is marked Done or Cancelled

**Feature dir**: `specs/002-auto-archive-terminal/`
**Status**: Clarified (no open questions)
**Task**: `vk/2f63-auto-archive-wor`
**Scope (Constitution)**: `crates/remote` (remote/cloud server) — backend only.

## Summary
On the remote/cloud server, moving an issue into the **Done** status already
auto-archives that issue's still-active workspaces, so completed work stops
occupying the workspace sidebar and enters the accelerated cleanup window.
Cancelling an issue does not do this today: its workspaces stay active even
though the issue is closed. This feature extends the existing behaviour so that
moving an issue into **either** terminal status — **Done or Cancelled** —
archives its active workspaces. Done's behaviour is unchanged; Cancelled is
newly covered.

## User Stories
- As a user who cancels an issue, I want its active workspaces to be archived
  automatically, so a cancelled issue behaves like a completed one and its
  workspaces stop cluttering my sidebar and start their faster cleanup.
- As a user who marks an issue Done, I want the current archive behaviour
  (including the "work not merged" warning in the server log) to keep working
  exactly as it does today.
- As a user, I do not want archiving to fire on non-terminal transitions (e.g.
  To do → In progress) or when the status did not actually change.

## Functional Requirements
- FR-1: When an issue's status changes to a **terminal** status — one whose name
  is "Done" or "Cancelled" (case-insensitive) — its still-active (non-archived)
  workspaces MUST be archived.
- FR-2: Archiving MUST commit atomically with the status change (same
  transaction), so a client that observes the status change also observes the
  archived workspaces (covered by the same `txid`).
- FR-3: The archive MUST NOT fire when the issue's status did not change, nor
  when the new status is non-terminal.
- FR-4: When the new status is **Done** and the issue's pull requests are not all
  merged, the server MUST log the existing "archiving, but PRs not all merged"
  warning. This warning is **not** emitted for **Cancelled** (cancelled work is
  intentionally abandoned).
- FR-5: If the issue has no active workspaces, the transition MUST succeed with
  no archive side effect and no error.
- FR-6: The behaviour MUST apply on both the single-issue update path and the
  bulk-update path.
- FR-7: A failure while loading status/workspaces or performing the archive MUST
  fail the whole update transaction (no partial state); a failure only while
  *loading pull requests for the warning* MUST degrade to "no warning" without
  failing the update (matching today's Done behaviour).

## Out of Scope
- The local (SQLite `Task`) deployment path — it has no equivalent archive hook;
  this mirrors where the existing Done feature lives.
- Any UI/frontend change — clients already render the `archived` flag streamed
  over the workspace shape.
- Un-archiving when an issue is reopened (moved back out of a terminal status).
- Introducing a first-class "terminal"/"category" flag on `project_statuses`;
  terminal statuses continue to be identified by name, as they are today.
- Migrations, new API endpoints, or generated-type changes.

## Acceptance Criteria
- [ ] Moving an issue with active workspaces from a non-terminal status to
      **Cancelled** archives all of its active workspaces.
- [ ] Moving an issue to **Done** archives its active workspaces exactly as
      before (behaviour and the unmerged-PR warning unchanged).
- [ ] Moving an issue between non-terminal statuses archives nothing.
- [ ] Re-saving an issue already in a terminal status (no status change)
      archives nothing.
- [ ] An issue with no active workspaces transitions to a terminal status with
      no error.
- [ ] Both the single update and bulk update endpoints exhibit FR-1..FR-7.
- [ ] A unit test covers terminal-status name recognition (Done/Cancelled,
      case-insensitive, incl. "Canceled"; negatives for non-terminal names).
- [ ] `cargo test -p remote`, `cargo clippy -p remote`, and `pnpm run format`
      pass.

## Notes / Resolved questions
- "Terminal status" set = {Done, Cancelled} matched by case-insensitive name,
  also accepting the American spelling "Canceled" → treated as Cancelled. This
  matches `DEFAULT_STATUSES` and the existing Done match; no clarification needed.
