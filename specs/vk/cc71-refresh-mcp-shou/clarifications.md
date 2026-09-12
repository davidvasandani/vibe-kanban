# Clarifications: Refresh Active Remote MCP Snapshots

`/speckit.clarify` resolved the specification's open questions from the VAS-356
snapshot path, the existing live-refresh contract, and the cluster ownership
model.

## C1. Authoritative settings and profile resolution

The coordinator remains authoritative. Refresh resolves the latest execution
profile with `latest_executor_profile_for_session`, then resolves that profile
through the same cached `ExecutorConfigs` machinery used by clustered dispatch.
The settings-owned MCP map is read with the existing
`read_coding_agent_mcp_servers` resolver. Refresh must not reconstruct profile
names, read a worker-global settings file, or treat the execution's old snapshot
as authoritative.

The resolved map is sent to the assigned worker as a bounded
`McpConfigSnapshot`, matching the VAS-356 dispatch contract.

## C2. Target identity and isolation

The latest active Codex execution for the session is the refresh target. The
coordinator uses persisted workspace placement and execution-worker-job data to
route the request to that execution's assigned worker. The worker authorizes the
execution ID and rematerializes only that execution's scoped Codex home.

No user-global Codex home is written. Each execution-scoped home continues to be
keyed by execution ID, preserving concurrent-session isolation.

## C3. Busy semantics

The existing `McpRefreshCoordinator`, keyed by session ID, remains the
authoritative generation and contention gate. A second request while a
generation is pending returns the existing retryable `busy` result. The worker
also serializes rematerialization and reload for one execution so coordinator
retries or races cannot interleave file replacement with reload.

No coordination lock is held across coordinator-to-worker network I/O or Codex
protocol I/O; the generation/claim is established first and external work runs
afterward.

## C4. Supported and unsupported sessions

In-place rematerialization is supported for active Codex app-server sessions
whose live control and VAS-356 scoped configuration are owned by the assigned
worker. Local Codex sessions keep their existing local live-refresh behavior and
use the same latest-settings resolution rule where a scoped snapshot exists.

Non-Codex executors remain `unsupported` because the repository has no
confirmable equivalent to Codex `config/mcpServer/reload`. A missing worker job,
missing live Codex control, legacy execution without a scoped home, or worker
that lacks the refresh protocol is reported truthfully as unsupported or a
reload/bootstrap failure; it is never reported as refreshed. This task does not
silently replace the user's active conversation with a new session.

## C5. Completion evidence

Materialization success is a prerequisite, not completion evidence. The worker
atomically replaces only the MCP section in the existing scoped `config.toml`,
then invokes `config/mcpServer/reload` on that execution's live Codex control.
The existing next-turn status enumeration remains the confirmation boundary for
`refreshed` or `partially_refreshed`; failure before reload is categorized as
materialization, while protocol/bootstrap failures are categorized separately.

## C6. Worker smoke coverage

The worker-side smoke test uses a deterministic test MCP server, starts from
snapshot A, rematerializes snapshot B in the same scoped home, performs MCP
initialize plus `tools/list`, and verifies B is visible without replacing the
conversation/thread identity. The test must not depend on network services or
real credentials.

## Remaining questions

None.
