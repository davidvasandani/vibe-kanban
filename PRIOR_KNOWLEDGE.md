# Prior knowledge — vk/1e1c-don-t-auto-archi

Searched homelab/knowledge-base, homelab/docs/knowledge, homelab/docs/knowledge-base, vibe-kanban/wiki and vibe-kanban/docs/knowledge-base for archive, terminal issue, poller and wake-up.

- vibe-kanban/docs/knowledge-base/issue-status-side-effects.md: remote terminal status and archive writes share a transaction; ProjectProvider and LinkedIssueProvider both use level-triggered archive reconciliation. Keep optional WorkspaceContext behavior and do not archive from optimistic status. Existing reconciliation requires a mounted UI.
- vibe-kanban/wiki/issue-workspace-lifecycle.md: remote archived-to-active updates can reopen Done issues. Therefore refusal must retain remote archived=true and retry rather than blindly unarchive remotely. Local accepted activity already reactivates workspaces.
- vibe-kanban/wiki/durable-session-wakeups.md: metadata lives in wakeups; lifecycle is in issue_comment_deliveries. Pending and dispatching must be protected, including past-due timers and condition receipts. Explicit archive currently cancels pending delivery.
- vibe-kanban/wiki/vk-pollers.md: pollers are BackgroundHelper ScriptRequests with poller metadata, not a distinct run reason. History dropping does not stop them. Do not protect every helper.
- homelab/docs/knowledge-base/remote-browser-preview-leases.md: archiving revokes leases as well as stopping helpers, so eligibility must precede cleanup.

Decision: guarded automatic endpoint; atomic local eligibility transition; no remote rollback, no new server subscription. Retry pending requests periodically while reconciliation is mounted. Manual archive is unchanged.
