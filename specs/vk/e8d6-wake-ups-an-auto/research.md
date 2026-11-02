# Research

- PR monitor watches transitions of open PRs and calls unconditional archive; git/pr merge handlers do likewise.
- Terminal issue remote hook runs only on status change. UI archive reconciliation is level-triggered, retries every 30 seconds, and re-reads remote state.
- Local automatic admission already checks running coding agents/pollers and pending/dispatching wake-ups atomically. Extend it rather than creating another eligibility implementation.
- Explicit unarchive currently writes SQLite then asynchronously syncs remote. Persist acknowledgement before async work to defeat stale requests.
- Schedule validates local SQLite, not remote cache. Existing immediate wake-up insertion serializes limits and delivery creation.
- Global MCP discards session identity; launch already injects workspace ID but not session ID. Working-directory startup fetch has a 500ms timeout and cannot establish exact originating session in global mode.
