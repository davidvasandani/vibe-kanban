# Clarifications: Settings-Owned MCPs in Every New Session

## Resolved Questions

### Which executors receive a snapshot?

Every dispatched executor with a native MCP configuration adapter, not a
hard-coded Claude/Codex/Gemini allow-list. This keeps the producer aligned with
the domain already supported by Vibe Kanban Settings.

### How does a worker isolate vendor configuration?

The worker builds an execution-scoped home overlay. It writes the target native
config inside the overlay and exposes the remaining home contents through
links, preserving vendor login/session assets without copying or mutating them.
Codex retains its narrower `CODEX_HOME` override because that is its established
configuration boundary; other executors receive the scoped `HOME` (and a scoped
XDG config root where applicable).

### Does this add live refresh for every executor?

No. Live refresh remains Codex-only until an executor protocol can confirm
in-process adoption. All supported executors receive the latest Settings state
at their next child-process start.

### Are saved header values converted into environment variables?

No. The coordinator sends the already-adapted native definition, including
literal headers, over the authenticated bounded snapshot channel. The worker
does not synthesize or persist deployment environment variables.

### What repository configuration is removed?

Only the competing `vibe-kanban` server entry in `homelab/.mcp.json`. All other
project MCP entries remain untouched. Settings identifier `vibe_kanban` and its
display label remain authoritative.

### How are screenshot-exposed credentials handled?

They are not copied into code, tests, documentation, logs, or task artifacts.
The bearer token and both Cloudflare Access credentials must be rotated through
the operator's secret-management workflow after this change.

## Remaining Open Questions

None that block implementation.
