# Durable wake-ups across workspace archival

## Problem and evidence
Follow-up work must remain schedulable after merge or issue completion. The current local wake-up API rejects the local Workspace archived flag; it does not read a remote cache. Explicit edits update that same SQLite store. Remote terminal-issue archival runs on status transitions, while local PR monitoring and merge routes still call unconditional archive. Remote archive reconciliation has activity guards already, but explicit unarchive does not persist an acknowledgement of the archive trigger.

The October 25 incident's exact actor cannot be proved from current source alone. Record archive provenance so subsequent incidents are attributable.

## Required behavior
- Every automatic archive path protects active coding turns, running pollers, and pending or dispatching wake-ups.
- Explicit unarchive acknowledges existing archive intent; repeated reconciliation cannot undo it. A genuinely new terminal-issue transition or merge may archive again.
- Scheduling validates time, prompt, ownership, worktree availability, and limits, then atomically persists the wake-up and reactivates an archived workspace. Scheduling is a temporary hold, not permanent dismissal of completion intent. When the last continuation finishes or is cancelled, automatic archive can resume. Activity activation and metadata synchronization must preserve unacknowledged completion intent and must not reopen the remote Done issue.
- Store archive rule, timestamp, and trigger identity. Expose provenance through workspace APIs, MCP listing, and UI. Errors for unavailable worktrees identify archive context and recovery action.
- Workspace agents inherit their own workspace and session identity; global external clients without that identity must supply explicit targets. Never guess the latest session.

## Implementation boundaries
Only the Vibe Kanban repository and, if required for its context propagation, its homelab deployment configuration are in scope. Preserve explicit user archive semantics, delivery admission, timer limits, and prompt durability. Centralize automatic archive admission in the local database and cover races with transactional state changes. Preserve separate remote intent and local runtime authority.

## Validation
Regression coverage must include merge then scheduling, issue Done plus pending timer, unarchive against stale archive requests, running poller and active turn holds, last wake-up release, missing worktree diagnostics, exact originating-session inference, and delayed dispatch of the stored prompt. Run repository formatting and appropriate Rust/frontend verification, independent Codex review, record reusable knowledge, then open and merge a PR after checks pass.
