# Feature Specification: Serialized, time-bounded branch mirror to the source checkout

**Feature dir**: `specs/vk/2b38-vas-1140-seriali/`

**Created**: 2026-11-28

**Status**: Draft

**Input**: "Serialize and time-bound the workspace branch mirror push from the
shared cluster store to the operator's source checkout (VAS-1140). Make it
single-flight per (repo, branch), add a timeout with process-group kill, add
backoff, show failures on the workspace, and clean up stale worktree
registrations in the source repo on adopt, delete and startup. Add an
optional warn-only large-commit guard."

## Background

For a clustered workspace, Vibe Kanban keeps a courtesy copy of the workspace
branch in the operator's own checkout (`/srv/src/<repo>`). This keeps the
branch visible to tools and older releases that look there. On 2026-11-28 a
workspace branch holding 1.6 GB of agent-committed files caused 44 concurrent,
never-ending copy attempts. They pushed the coordinator to load 127, made the
UI return 502s, and left every follow-up in that session stuck at "Sending".
Every attempt would have been refused anyway: a leftover registration of a
deleted worktree in the operator checkout still "held" the branch.

## User Scenarios & Testing *(mandatory)*

### User Story 1: Using a session never piles up background copies (Priority: P1)

An operator works in a clustered workspace. Each action (sending a follow-up,
opening the diff, checking git status) may ask for the branch to be copied to
the operator checkout. Whatever the branch's size, at most one copy runs for
that branch at a time. The operator's action never waits for the copy.

**Why this priority**: this was the outage. It is the minimum fix that restores
service.

**Independent Test**: trigger the copy repeatedly while one is slow. Count the
copy processes (exactly one) and confirm each trigger returns at once.

**Acceptance Scenarios**:

1. **Given** a copy for branch B is in progress, **When** ten more triggers for B
   arrive, **Then** no new copy process starts, and at most one follow-up copy
   runs after the current one finishes.
2. **Given** the operator checkout already has the same commit for B, **When** a
   trigger arrives, **Then** no copy process is started at all.
3. **Given** any copy state, **When** the operator sends a follow-up, **Then** the
   follow-up is not delayed by the copy.

---

### User Story 2: A copy that hangs is stopped, children included (Priority: P1)

**Why this priority**: without a bound, one bad branch can still hold
resources forever, even when only one copy runs.

**Independent Test**: use a copy that never finishes and spawns a child.
Confirm that both the copy and its child are gone shortly after the timeout,
and that a timeout failure is recorded.

**Acceptance Scenarios**:

1. **Given** a copy that does not finish, **When** the timeout elapses, **Then** the
   copy and every process it started are terminated and a "timed out" failure
   is recorded.

---

### User Story 3: A refused copy is visible and is not retried in a loop (Priority: P2)

**Why this priority**: silent failure hid the root cause for 77 minutes.

**Independent Test**: make the destination refuse the branch, for example
because it is checked out there. Confirm that the log line contains git's
reason, that the workspace's repository card shows the error, and that later
triggers do not start new attempts until the backoff expires and the branch
has moved.

**Acceptance Scenarios**:

1. **Given** the destination refuses B, **When** the copy fails, **Then** a warning is
   logged with git's error text, and the workspace's repo status shows the
   failure and when it will be retried.
2. **Given** a deterministic refusal for commit C, **When** further triggers
   arrive while B is still at C, **Then** no new copy process is started.
3. **Given** a transient failure, **When** triggers keep arriving, **Then** retries are
   spaced with growing delays up to a cap.
4. **Given** a later copy succeeds, **Then** the failure disappears from the
   workspace and a recovery line is logged.

---

### User Story 4: Removing or moving a workspace leaves no registration behind (Priority: P2)

**Why this priority**: the stale registration is what turned a slow copy into
one that could never succeed. 119 more of them had built up.

**Independent Test**:
- Adopt a worktree from the operator checkout into the shared store, then
  check that the checkout no longer lists it.
- Delete a clustered workspace, then check that the checkout lists no
  prunable entry for it.
- Start the server with existing stale entries, then check that they are gone
  and that live entries remain.

**Acceptance Scenarios**:

1. **Given** a worktree adopted into the shared store, **Then** its old
   registration in the operator checkout is removed.
2. **Given** a clustered workspace is deleted, **Then** the operator checkout has no
   registration pointing at its worktree.
3. **Given** the server starts, **Then** registrations whose worktree is definitely
   gone, or definitely re-linked elsewhere, are removed. Registrations whose
   state cannot be determined, that are locked, or that belong to a live
   workspace are kept.
4. **Given** a stale registration holds branch B when a copy is attempted, **Then** it
   is removed and the copy proceeds. A live registration holding B instead
   produces a visible deterministic failure without starting a copy.

---

### User Story 5: An oversized agent commit is flagged (Priority: P3)

**Why this priority**: an optional early warning for generated output.

**Acceptance Scenarios**:

1. **Given** an agent auto-commit adds more than the file-count or size threshold,
   **Then** a warning naming the workspace, the counts and the thresholds is
   logged. The commit itself is not refused.

### Edge Cases

- The storage holding workspace directories is not mounted. A missing
  worktree must not be read as "deleted", so its registration is kept.
- The destination repo is unreadable or missing. This is a recorded failure,
  not a crash.
- The process restarts. Failure state is in memory and is re-established by
  the next attempt.
- The branch is only a remote-tracking ref in the store. Nothing is copied,
  which is the existing behaviour.
- The copy is disabled by configuration. No copies run and no failures are
  shown.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: At most one copy process MUST run per (destination checkout,
  branch) at any time.
- **FR-002**: A trigger during an in-flight copy MUST NOT start a process. It
  MUST cause at most one follow-up copy once the current copy succeeds.
- **FR-003**: Triggers MUST return without waiting on git.
- **FR-004**: A copy MUST be skipped when the destination already holds the
  store's commit for the branch.
- **FR-005**: Every copy MUST have a timeout (default 5 minutes,
  operator-configurable). On expiry the copy's whole process group MUST be
  terminated.
- **FR-006**: Failures MUST be classified as deterministic (the destination
  refused), transient, or timeout, and logged at warning level with a bounded
  excerpt of git's error output.
- **FR-007**: Retries MUST back off exponentially (base 1 min). The cap is
  30 min for transient failures and timeouts, and 6 h for deterministic ones.
  A deterministic failure MUST NOT be retried while the branch is still at the
  commit that failed.
- **FR-008**: The current failure, if any, MUST be shown per repository on the
  workspace's git status, with its message and next retry time. It MUST clear
  on success.
- **FR-009**: Adopting a worktree into the shared store MUST remove its old
  registration from the operator checkout.
- **FR-010**: Deleting a clustered workspace MUST remove the operator
  checkout's registrations that point at that workspace's worktrees.
- **FR-011**: On startup, stale registrations in every registered repository's
  checkout MUST be removed. Removal requires positive evidence and is done per
  entry, never by a blanket prune (Constitution XV, LXI).
- **FR-012**: Before copying, a stale registration holding the branch MUST be
  removed. A live one MUST produce a deterministic failure without a copy.
- **FR-013**: The copy MUST be disableable by configuration.
- **FR-014** (optional): Agent auto-commits exceeding configurable file-count
  or size thresholds MUST log a warning. They MUST NOT be refused.

### Key Entities

- **Mirror slot**: per (destination, branch). Holds whether a copy is in
  flight, whether a follow-up was requested, and the last failure
  (kind, message, attempts, last attempt time, next allowed time, failed
  commit).
- **Registration**: an entry in a checkout's worktree administration area. It
  names a worktree path and the branch it holds. Its state is live, stale or
  indeterminate.

## Success Criteria *(mandatory)*

- **SC-001**: In the reproduction, one branch triggered 100 times while a copy
  is slow, at most 2 copy processes are ever started (the current one plus one
  follow-up), and never more than 1 at a time.
- **SC-002**: A hung copy and its children are gone within timeout + 5 s.
- **SC-003**: A refused copy appears in the logs and in the workspace UI within
  one status poll. Repeated triggers on an unchanged branch start zero further
  processes.
- **SC-004**: After a workspace delete, a server start, or an adoption, the
  operator checkout lists zero prunable entries attributable to Vibe Kanban
  workspaces. Live and indeterminate entries are untouched.

## Assumptions

- Failure state does not need to survive a restart.
- A stale registration is safe to remove when either:
  - its worktree directory is definitely absent and the directory above the
    workspace (the storage) is present; or
  - the worktree's `.git` points to a different registration.
- The warn-only commit guard defaults to 5000 files or 200 MB.

## Clarifications

### Session 2026-11-28

There were no `[NEEDS CLARIFICATION]` markers. The ambiguous areas below were
resolved from the task text, the code and the constitution. No questions
remain open.

- **Q: Skip or merge a trigger during a running copy?**
  A: Merge. Set a follow-up flag and run one more copy after a success, so the
  newest commit always lands. No process is started for the trigger itself
  (FR-002).
- **Q: Which repo state does the UI show the failure on?**
  A: The existing per-repository git status that the workspace's Git panel
  polls. It is shown as a warning line on the repo card (FR-008). No new
  endpoint.
- **Q: Does "stop retrying a deterministic error forever" mean never retry?**
  A: No. Retry only once the branch has moved to a new commit **and** the
  backoff has elapsed. A rejection caused by a stale registration is fixed by
  FR-012, so a later attempt can succeed.
- **Q: Should startup run `git worktree prune`?**
  A: No. The codebase deliberately avoids blanket prune, because over NFS it
  can unregister live worktrees. Use per-entry removal on positive evidence,
  plus a guard that skips entries under live workspace directories.
- **Q: Warn or refuse on a large commit?**
  A: Warn only (FR-014). Refusing would block an agent turn on a heuristic.
- **Q: Timeout default?**
  A: 5 minutes, configurable through the environment. It must still
  accommodate normal local-to-local pushes on NFS.
- **Q: Which copies are covered: only workspace branches, or base branches
  too?**
  A: Both. Every call to the courtesy mirror goes through the same
  single-flight path.
