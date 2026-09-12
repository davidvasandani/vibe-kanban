# Clarifications: Refresh Active Workspace MCP Inventories

## 1. What is the supported adoption boundary?

**Decision:** Use the already-shipped explicit agent restart as the correctness
boundary for user-facing refresh. The normal follow-up path starts a fresh
executor process from the latest native MCP configuration while preserving the
workspace and conversation identity. A running turn finishes normally after
confirmation, then the queued synthetic continuation starts the fresh process.

The pinned Codex app-server exposes `config/mcpServer/reload` and documents
next-active-turn application, but its empty acknowledgement does not prove the
model-visible schema changed. `mcpServerStatus/list` enumerates server status
and tools but exposes no inventory generation. The internal live-refresh API may
remain for supported callers, but the UI must not use its acknowledgement or
counts as the stronger acceptance boundary.

## 2. Is the defect stdio-only?

**Decision:** The reported stale inventory is primarily a stdio process
lifecycle issue: a changed executable or `tools/list` implementation requires a
new server process/connection. A fresh coding-agent process provides that
boundary. Remote HTTP MCP servers may also change their advertised tools, but
this task will not impose stdio restart semantics on them without evidence.
Regression coverage will prove current non-stdio configuration survives the
restart path and remains assigned.

## 3. What does “not installed” mean for Personal ServiceNow?

**Decision:** A Personal ServiceNow MCP assignment is not a Codex plugin
installation. The Vibe Kanban connector surface must report its native
assignment, enablement, and connectivity inventory using the MCP server's stable
identifier. A plugin-manager lookup may truthfully say no plugin bundle is
installed, but that status must not be presented as the MCP connector's
effective installation state. No marketplace/plugin redesign is included.

## 4. Is refresh automatic after settings save?

**Decision:** Keep refresh explicit. Settings are shared across profiles and a
save may affect multiple running sessions; automatically restarting or queueing
all of them would be a materially broader lifecycle action. The workspace chat
toolbar's “Restart agent for MCP changes” action is the supported one-click
operation. Its label and confirmation must remain explicit that a fresh agent
process will start.

## 5. What must automated coverage demonstrate?

**Decision:** Exercise the fresh-process configuration boundary with three
successive stdio inventories: tool addition, removal, and same-name input-schema
replacement. Assert exact tool definitions reaching the next-turn runtime, not
only `tools/list` counts. Add a remote-transport configuration regression and
preserve the existing running/idle restart handoff tests.

## Remaining questions

None.
