# Feature Specification: Legacy MCP Identifier Migration

## User problem

Settings created before identifier validation may store a display label as the
native MCP key. The editor can display that server but cannot safely save it,
and restarted agents do not load it.

## Requirements

- Detect invalid native identifiers while loading shared MCP settings.
- Propose the existing canonical identifier and preserve the legacy key as the
  display label.
- Apply migration only through an explicit shared-config save.
- Preserve definitions and assignments exactly.
- Reject canonical-name collisions and cross-profile ambiguity before writes.
- Commit native key renames and display-label metadata as one recoverable
  operation with secret-safe diagnostics.

## Success criteria

- `Atlassian Rovo` becomes `atlassian_rovo` and remains visibly labelled
  “Atlassian Rovo”.
- Restarted Codex sessions load the migrated server.
- Tests prove preservation, collision refusal, and no partial writes.
