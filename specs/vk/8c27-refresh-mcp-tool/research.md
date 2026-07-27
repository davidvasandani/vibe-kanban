# Research: Active MCP Refresh

## Existing ownership boundaries

- Executor-native MCP config files are the source of truth
  (`crates/executors/src/mcp_config.rs`).
- `crates/executors/src/mcp_test.rs` can independently initialize MCP servers
  and list tools, but that probe does not mutate or confirm a coding agent's
  live inventory.
- `LocalContainerService` owns live coding-agent processes and maps execution
  IDs to process/cancellation/log handles.
- Codex's `AppServerClient` is created inside the executor's spawned task and is
  currently not registered with the container.

## Pinned Codex protocol

VK pins `@openai/codex@0.144.1` and the matching
`codex-app-server-protocol` tag `rust-v0.144.1`.

The protocol exposes:

- `config/mcpServer/reload` (`ClientRequest::McpServerRefresh`): re-reads config
  from disk and queues an MCP refresh for loaded threads;
- `mcpServerStatus/list`: enumerates configured servers, tools, auth state, and
  optionally resources/resource templates.

The reload acknowledgement is empty. Codex applies a queued refresh on each
thread's next active turn, rebuilding its MCP connection manager off to the side
and replacing the manager after construction. This is the required atomic
owner-confirmed boundary.

## Lifecycle constraint

Codex app-server processes are currently one turn long (`keep_warm: false`).
During a running turn, VK can send reload through the live `AppServerClient`;
Codex queues it without interrupting the turn. If the process exits before
another turn, the next Codex app-server naturally reads current config at
startup. VK must carry a pending refresh generation at the session level and
confirm it from the next execution's status before showing success.

This avoids expanding scope into Codex keep-warm, which the lifecycle knowledge
base explicitly defers.

## Other executors

No equivalent confirmed in-session reload contract is present for other
executors. OpenCode exposes `GET /mcp` status in VK's SDK wrapper but no reload
operation. ACP-based executors and one-shot CLI executors likewise have no
registered live control operation. They remain capability-gated unsupported.

## Reuse decisions

- Reuse Codex's own config parser, reconciliation, connection startup,
  next-turn serialization, and atomic connection-manager replacement.
- Reuse `mcpServerStatus/list` instead of adding a second VK-owned MCP client for
  successful confirmation.
- Reuse existing config adapters and native-file source of truth.
- Reuse/redesign existing `mcp_test.rs` redaction helpers only for public
  diagnostics; do not return raw Codex errors.
- Add no top-level dependency.

## Result-model limitations

Codex chooses which individual connections can be reused/restarted internally,
but its public reload response/status list does not expose a per-server
`restarted` bit. VK can report:

- `reload_queued` for the request;
- `ready`/`failed` plus tool count after confirmation;
- `restart_occurred: unknown` unless the pinned protocol is extended.

The UX must show unknown honestly. It must not infer restart from config text
because Codex owns normalization and connection reuse.

## Slack regression method

The pinned Slack entry already targets `v1.3.0-vk.2`. A regression fixture can
use a deterministic stdio mock named Slack for normal CI, while an ignored
integration test uses isolated npm/launcher caches and the pinned release asset.
The live Codex status must contain `attachment_get_data`; an independent probe
is useful only as artifact diagnosis, not acceptance evidence.
