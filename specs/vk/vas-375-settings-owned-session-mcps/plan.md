# Technical Plan: Settings-Owned MCPs in Every New Session

## Architecture

The coordinator remains the settings authority. At dispatch it reads the
selected executor profile through the existing MCP adapter and attaches the
native server map to `McpConfigSnapshot`. The worker validates the executor,
creates an isolated home overlay, writes the native map to the executor's normal
config location inside that overlay, and launches the agent with an environment
override pointing at the overlay.

Codex keeps its current `CODEX_HOME` boundary and confirmed refresh path. Other
executors use a scoped `HOME`; executors whose native path follows XDG also use
the overlay's `.config` directory. Unrelated home assets are linked into the
overlay so vendor authentication and continuation data remain available.

## Implementation Steps

1. Generalize coordinator snapshot creation from Codex-only to every executor
   for which the selected coding-agent configuration supplies a native MCP map.
2. Refactor worker preparation around a generic scoped-home result containing
   execution root, native target path, executor, and launch environment.
3. Build safe home overlays that exclude the target config while preserving
   siblings and recursively overlaying its ancestor directories.
4. Materialize the snapshot through
   `write_coding_agent_mcp_servers_to_path`, retaining atomic native writes and
   unrelated settings from the source config.
5. Apply the scoped environment only to the launched execution and point Codex
   refresh at the stored native target path.
6. Add focused tests for producer coverage, native-path overlay behavior,
   concurrent isolation, cleanup, mismatch rejection, and Codex refresh.
7. Remove only the competing Vibe Kanban server from `homelab/.mcp.json`.
8. Format and run affected checks, then independently review the complete diff.

## Verification

- `cargo test -p local-deployment` for dispatch snapshot behavior.
- `cargo test -p worker` for execution-scoped materialization and refresh.
- `cargo fmt --all -- --check` and repository formatting.
- Parse `homelab/.mcp.json` and assert unrelated MCP definitions remain.
- Search diffs and test output for accidental credential material.

## Rollout and Rollback

The protocol field is already optional, so coordinator and workers retain
rolling compatibility. Deploy coordinator and workers together for immediate
all-executor behavior. Rollback restores the prior binaries; the removed
repository definition should not be restored because Settings remains the sole
authority.
