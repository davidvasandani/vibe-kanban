# Feature Specification: Restart Agents After MCP Changes

**Feature dir**: `specs/vk/mcp-agent-restart/`
**Status**: Clarified

## Summary

Replace the Codex-only MCP reload toolbar behavior with a fresh-process
continuation that works for every agent type.

## Requirements

- Stopped selected sessions start a follow-up immediately.
- Running selected sessions show a confirmation before queueing a follow-up.
- Cancel is inert and confirm does not interrupt the current turn.
- Existing queued user input is preserved and itself becomes the restart turn.
- Running state is scoped to the selected session and coding-agent processes.
- The logical session and executor-specific continuation identity are retained.

## Acceptance criteria

- Unit tests prove immediate, cancel, queue, and preserve-existing behavior.
- Web-core typechecking and formatting pass.
- The toolbar names the operation as a restart, not a live refresh.
