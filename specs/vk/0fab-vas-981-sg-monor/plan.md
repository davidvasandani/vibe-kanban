# Implementation Plan: Reliable workspace creation for large/foreign-hook repos

**Spec**: `./spec.md`
**Status**: Implemented (see Risks & Dependencies for remaining verification)

## Technical Context
Rust workspace. Affected crates: `git` (CLI wrapper around the `git`
executable), `workspace-manager` (workspace/repo attach logic shared by
coordinator and worker placement), `server` (HTTP routes for workspace
creation and session creation), `db` (already has the `creation_status`/
`creation_error` columns this plan relies on — no migration needed). No new
dependency, no schema change, no new API shape — only new/changed error
messages and one new internal method.

## Architecture & Approach
- **FR-1 (fetch remote-only branch):** `GitService::fetch_remote_branch`
  (new, `crates/git/src/lib.rs`, next to `check_remote_branch_exists`) wraps
  the existing `GitCli::fetch_with_refspec`. `WorkspaceManagedWorkspace::
  add_repository` (`crates/workspace-manager/src/workspace_manager.rs:244`)
  calls it when `check_branch_exists` is false and
  `check_remote_branch_exists` is true, then re-checks before raising
  `BranchNotFound`. This reuses the exact pattern already used in
  `crates/server/src/routes/workspaces/pr.rs` for target-branch resolution,
  so there's one convention for "does this branch exist, possibly only on
  the remote" rather than a second one (Constitution XXI).
- **FR-2 (hook isolation):** `GitCli::git_impl` (`crates/git/src/cli.rs`,
  the single function every `GitCli` command funnels through) now passes
  `-c core.hooksPath=/dev/null`. This is a one-line, one-location change
  that covers every current and future `GitCli` call site, matching the
  existing per-call precedent in `crates/remote/src/github_app/service.rs`
  but centralized (Constitution XXXI).
- **FR-3 (real failure message persisted):** `create_and_start_workspace`'s
  background task (`crates/server/src/routes/workspaces/create.rs:560`)
  passes `error.to_string()` to `fail_creation_with_retry` instead of a
  hardcoded placeholder. `Workspace::fail_creation` already writes this into
  the existing `creation_error` column — no new storage needed.
- **FR-4 (create_session respects creation_status):** `create_session`
  (`crates/server/src/routes/sessions/mod.rs`) matches on
  `workspace.creation_status` after the existing `find_by_id` check:
  `Ready` proceeds, `Failed` returns `creation_error`, `Queued`/`Running`
  returns a "still being created" message. All three are distinct
  `ApiError::Workspace(WorkspaceError::ValidationError(..))` messages, so
  this doesn't require a new `ApiError` variant or wire-type change.

## Data Model
No changes. `Workspace.creation_status` (`WorkspaceCreationStatus`: Queued /
Running / Ready / Failed) and `Workspace.creation_error: Option<String>`
already exist (`crates/db/src/models/workspace.rs`) and already round-trip
through `ts-rs` to the frontend types; this plan only changes what gets
written into `creation_error` and adds a new read of `creation_status` in
one more place.

## Contracts
No new endpoint and no request/response shape change. `create_session`'s
error *message* changes (more specific), but its status code and response
envelope (`ApiError` → existing error JSON shape) are unchanged, so no
generated-type regeneration is required.

## Research Notes
Considered polling/awaiting the background creation task from the
`create_and_start_workspace` HTTP handler itself (so `start_workspace` could
return the real error synchronously for fast failures like branch-not-found,
while still returning early for slow-but-healthy creations like sg-monorepo's
~1-minute NFS checkout). Rejected for this change: it would turn a
fire-and-forget spawn into a partially-synchronous one, adding a new
timeout/cancellation surface for comparatively small benefit, since FR-3+FR-4
already make the failure reason reachable on the very next call
(`create_session`) instead of only in server logs. Left as a possible future
enhancement, not required by any acceptance criterion in `spec.md`.

## Constitution Check
- Principle XXI ("failures say what failed"): FR-3 is a direct instance —
  replacing a generic placeholder with the real error's message.
- Principle LIX (new, this task — the constitution was at LVIII/0.55.0 when
  this task started a rebase onto current `main`; renumbered accordingly):
  FR-1 and FR-2 are exactly its two clauses (remote-fetch-before-fail;
  hook-isolated git plumbing).
- Principle VI ("don't rebuild what shipped"): FR-1 reuses
  `check_remote_branch_exists`/`fetch_with_refspec`, already present for the
  PR-target-branch case, rather than inventing a parallel fetch path.
- No deviation identified.

## Risks & Dependencies
- The hook-isolation fix (`core.hooksPath=/dev/null`) applies to every
  `GitCli` command; if any existing code path *relied* on a repo's own
  hooks running (none found in this codebase), this would be a behavior
  change. Mitigated by `cargo check`/`cargo test` across `git`,
  `workspace-manager`, and `server` and a targeted read of all `GitCli`
  call sites before merging.
- `fetch_remote_branch` depends on the repo's registered remote being
  reachable at workspace-creation time; a genuinely offline/unreachable
  remote now surfaces as a `GitService`/`GitCliError` (e.g. auth or network
  failure) rather than the previous generic `BranchNotFound`, which is the
  intended, more specific behavior per FR-1/Constitution XXI but is a
  message-content change worth calling out in the PR description.
- Live re-verification against the actual sg-monorepo
  `feat/linux-pos-prototype` branch requires this branch to be deployed to
  the host running the shared clone; not done as part of this plan (see
  spec.md "Out of Scope" — host filesystem state is unaffected by this plan).
