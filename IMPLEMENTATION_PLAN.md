# Implementation Plan: Active Workspace MCP Refresh

This plan builds on `SPEC.md` and `PRIOR_KNOWLEDGE.md`. It deliberately starts
with executor protocol discovery because VK currently owns configuration files
and process lifecycle, while each coding agent owns its live MCP inventory.

## 1. Establish supported executor reload contracts

1. Inventory every MCP-capable executor and its turn/session process lifecycle.
2. Inspect pinned CLI protocols and local client code for a live operation that:
   - reloads native MCP configuration;
   - waits for server initialization/capability listing; and
   - confirms the inventory generation used by the next turn.
3. Record a capability matrix for Claude, Codex, OpenCode, ACP/Grok, Gemini,
   Cursor, Amp, Copilot, Droid, and Qwen.
4. Select only adapters with a verifiable in-session contract. Mark the rest
   explicitly unsupported; do not substitute the existing independent MCP probe.
5. Confirm how the Slack regression will be exercised against a supported live
   executor and the pinned `v1.3.0-vk.2` artifact.

## 2. Define domain types and contracts

1. Add shared Rust/TypeScript types for:
   - overall refresh status;
   - per-server outcome;
   - capability counts;
   - restart/reuse indication;
   - classified safe errors;
   - refresh timestamps and inventory generation.
2. Define the `ContainerService`/executor refresh capability with a default
   unsupported implementation.
3. Add a session-scoped request/response service contract that validates active
   workspace/session ownership.
4. Register generated types and schemas; regenerate checked-in outputs.

## 3. Build secret-safe refresh diagnostics

1. Extract or reuse the existing MCP diagnostic redaction primitives.
2. Normalize configurations into comparison inputs whose secret-bearing values
   are represented only by keyed digests.
3. Map process, protocol, HTTP, timeout, authentication, schema, busy, and
   unsupported failures into stable categories and safe remediation.
4. Add tests for tokens in env values, headers, URLs, query strings, command
   arguments, stderr, and response bodies.

## 4. Implement the live refresh coordinator

1. Add per-session coordination state in the live container/executor owner:
   - refresh mutex/state;
   - immutable published generation;
   - last successful refresh time;
   - per-server last known-good metadata;
   - active-call/reference tracking where the adapter exposes it.
2. Reject or serialize concurrent refreshes with a retryable result.
3. Re-read the active executor's native config and diff by configured server ID
   and secret-safe fingerprint.
4. Delegate changed-server reconnect/restart and capability discovery to the
   executor adapter.
5. Build a complete candidate result, retaining last known-good data for failed
   servers and excluding explicitly removed/disabled servers.
6. Atomically publish only after the adapter confirms the agent adopted the
   candidate inventory.
7. Retire affected connections only after calls on the old generation finish,
   or return `busy_active_call` when safe deferral is unavailable.
8. Clean coordinator state on session/workspace teardown and application
   shutdown.

## 5. Implement supported executor adapters

For each adapter proven in step 1:

1. Add the vendor-specific reload/reconnect call using its existing authenticated
   transport and live process handle.
2. Translate native per-server states into the shared result model.
3. Confirm tools, resources, and prompts (where available), including schema
   validation.
4. Confirm the generation/status returned to VK matches what subsequent turns
   use.
5. Preserve unchanged healthy connections if the vendor API supports granular
   reconciliation; otherwise truthfully report which servers restarted.
6. Cover absent process, idle warm process, active call, process death, timeout,
   malformed response, partial failure, and authentication recovery.

## 6. Expose the REST API

1. Add
   `POST /api/workspaces/{workspace_id}/sessions/{session_id}/mcp/refresh`.
2. Reuse existing workspace/session lookup and authorization conventions.
3. Return typed busy, unsupported, partial, and success responses with appropriate
   HTTP status/envelope behavior.
4. Add route tests for scope, inactive sessions, concurrent refresh, and
   redaction.

## 7. Expose the VK MCP tool

1. Add `refresh_mcp_tools` to the Vibe Kanban MCP server.
2. In orchestrator mode, default to and enforce the scoped workspace/session.
3. In global mode, require explicit identifiers.
4. Call the same backend endpoint/service as the UI and return the structured
   safe result.
5. Update router membership and tool-schema tests.

## 8. Build the workspace UI

1. Add the API client mutation and query types.
2. Place **Refresh MCP tools** with active session controls.
3. Disable duplicate submission and discard stale responses after session
   switches/unmounts.
4. Render overall state, last confirmed successful time, per-server state, tool
   count, restart/reuse state, and safe remediation.
5. Never update displayed success metadata until the published-generation
   confirmation is present.
6. Add component/hook tests for success, partial failure, busy retry,
   unsupported, and stale-response suppression.

## 9. Test protocol and atomicity behavior

1. Create deterministic mock stdio and streamable-HTTP MCP servers whose tool
   sets can change from A to A+B and back.
2. Test add/remove/enable/disable, unchanged reuse, changed restart, credential
   renewal, partial failure, timeout, malformed `tools/list`, and schema errors.
3. Test an in-flight call racing refresh and concurrent refresh attempts.
4. Assert readers observe only complete old/new inventory generations.
5. Assert a failed changed server retains last known-good tools, while explicit
   disable removes them.
6. Assert removed tools produce the executor's clear unavailable-tool result.

## 10. Verify the Slack regression

1. Use isolated npm and launcher caches.
2. Start a supported active workspace session against an older/different Slack
   inventory.
3. update the native config to the pinned fork release `v1.3.0-vk.2`;
4. invoke refresh through the same session;
5. verify the confirmed inventory includes `attachment_get_data`;
6. invoke it with a deliberately incomplete safe payload and assert the handler
   validation response rather than `unknown tool`;
7. verify conversation/session/workspace identifiers did not change.

## 11. Documentation and rollout

1. Document supported executors, busy semantics, partial failure, secret-safe
   diagnostics, and operator remediation.
2. Add structured telemetry for duration, outcome, transport, restart/reuse,
   capability delta, and safe failure category.
3. Gate incomplete adapters and ensure the UI communicates unsupported status.
4. Update the project knowledge base with the proven live-reload architecture
   and executor capability matrix.

## 12. Verification

1. Install dependencies in the fresh worktree with
   `pnpm install --frozen-lockfile`.
2. Run focused Rust and frontend tests throughout implementation.
3. Regenerate and check shared types/schemas.
4. Run `pnpm run format`, relevant lint/check commands, and the broad workspace
   test suite in proportion to runtime.
5. Run independent Codex diff review, fix confirmed findings, and repeat until
   there are no significant findings.
