# Verification: Refresh Active Workspace MCP Inventories

Verified on 2026-09-03.

## Setup and formatting

- `pnpm install --frozen-lockfile`: passed.
- `pnpm run format`: passed.

## Focused contract tests

- `cargo test -p executors mcp_inventory_tests --lib`: 3 passed. Covers stdio
  add/remove/same-name schema-change generations plus stable map and nested JSON
  object ordering.
- `cargo test -p services mcp_refresh --lib`: 4 passed. Covers busy,
  unsupported, failure truthfulness, and complete exact-evidence replacement.
- `cargo test -p executors supports_streamable_http_assignments_for_codex --lib`:
  1 passed. Pins the non-stdio streamable-HTTP regression boundary.
- `pnpm --filter @vibe/web-core exec vitest run
  src/features/workspace-chat/model/restartAgentForMcpChanges.test.ts`: 6 passed.
  Covers idle start, running confirmation, queue handoff, race retry, and queued
  user-message preservation.

## Generated types and broader checks

- `pnpm run generate-types`: passed and updated `shared/types.ts`.
- `pnpm run generate-types:check`: passed.
- `cargo check -p worker -j1`: passed. Two earlier attempts were killed by the
  environment with signal 9 while default/parallel compilation consumed the
  available memory; the single-threaded check passed without a compiler error.
- `pnpm run check`: passed, including local-web, remote-web, web-core, UI, the
  complete main Rust workspace, and the remote Rust workspace.
- `cargo fmt --all --check`: passed.
- `cargo clippy -p executors --lib -- -D warnings`: passed after the independent
  review correction.
- `pnpm run lint`: frontend/UI lint passed; backend Clippy reached two existing,
  unchanged failures in `crates/services/src/services/entra_mint.rs:477`
  (`collapsible_if`) and `crates/services/src/services/cli_tools.rs:1468`
  (`single_element_loop`). Neither file differs from `origin/main` in this task.

## Evidence boundary

The pinned Codex protocol does not expose the private model request payload or
an inventory generation ID. Acceptance therefore uses its strongest public
post-start evidence: thread-scoped, full-detail `mcpServerStatus/list` after the
next turn starts. Sorted tool names prove additions/removals; the schema digest
proves same-name input/output schema replacement without returning full schemas.
