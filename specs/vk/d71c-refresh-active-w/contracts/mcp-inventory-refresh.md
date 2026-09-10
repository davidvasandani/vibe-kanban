# Contract: MCP Inventory Refresh for an Active Workspace

1. The user selects **Restart agent for MCP changes** in the existing workspace
   session.
2. If a coding-agent turn is running, the server requires explicit confirmation
   and lets that turn finish normally.
3. Exactly one lifecycle owner claims the synthetic continuation.
4. Any retained warm agent process is reaped.
5. The normal follow-up launch starts a fresh agent process using the selected
   profile and latest settings-owned native MCP configuration.
6. The process initializes its MCP connections and discovers current tools
   before the next model turn uses them.
7. One complete inventory replaces the old inventory: additions appear,
   removals disappear, and same-name schema changes replace the old schema.
8. The logical workspace/session and visible conversation remain intact.
9. A failure is visible and secret-safe; it is never reported as successful
   adoption based only on connector probe metadata.

For the internal live-refresh API, `pending_next_turn` means accepted but not
adopted. Only process-owned next-turn status can produce a terminal refresh
snapshot, and fields absent from the executor protocol remain unknown.
