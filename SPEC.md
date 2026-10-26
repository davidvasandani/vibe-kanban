# Guard remote-driven workspace auto-archive

Task: vk/1e1c-don-t-auto-archi

## Problem and scope
Remote issue completion (workflow signals and manual Done/Cancelled) archives remote workspace records without access to local activity. The UI currently applies that flag through the manual local update endpoint, stopping helpers, revoking previews, and causing scheduled wake-ups to be cancelled.

## Design
Add a dedicated server-enforced automatic archive endpoint and route the remote/local reconciliation hook through it. All clients use the same local database decision. Keep the existing manual update/archive endpoint behavior unchanged. A UI tab still initiates reconciliation; this change does not introduce a remote subscription or periodic server worker.

The server reads current local state and atomically marks the workspace archived only if it is unpinned, has no running coding-agent execution in any session, no running poller, and no wake-up whose delivery is pending or dispatching. The poller predicate matches ExecutionProcess::find_workspaces_with_running_pollers, including dropped history and valid ScriptRequest poller objects. Ordinary helpers and dev servers do not block. Due-but-undelivered wake-ups still block; delivered/cancelled/failed wake-ups do not.

A refused request leaves local resources intact and retains remote archived=true as pending archive intent. Return an explicit blocked outcome; the UI keeps displaying the local workspace as active and retries every 30 seconds while mounted. No remote unarchive is sent because it can reopen Done issues. Remount/reconnect also retries. The endpoint checks the current remote flag before applying a stale browser signal. Missing remote linkage or unavailable remote service never causes local archive. With no relevant UI mounted, existing reconciliation remains dormant; enforcement is server-side for every requesting client.

After a successful guarded local transition, use the existing archive cleanup for helpers, preview leases and archive scripts without a second unconditional archived write. Already archived workspaces are idempotent. Database errors fail closed. Serialize eligibility and local archived mutation in one database statement/transaction; no network call while holding the write transaction.

## Acceptance and verification
Tests cover pinned, running agent (including another session), running poller (including dropped history), pending and dispatching wake-up, completed/cancelled wake-up, plain helper, dev server, idle, already archived, missing workspace, and fail-closed database errors. Verify refusal never reaches cleanup, manual archival remains unconditional, stale remote=false is ignored, and remote read failures are surfaced. Frontend tests prove reconciliation uses the automatic endpoint and deduplicates in-flight requests. Run required install/format, focused Rust and frontend tests, applicable checks, independent Codex review, update knowledge, and create/merge PRs through Vibe Kanban after CI passes.

## Boundaries
No remote schema change, pinned-state replication, generalized helper blocking, or changes to explicit user archive. This guards archive admission at the database transition; future agent starts keep their existing reactivation behavior.
