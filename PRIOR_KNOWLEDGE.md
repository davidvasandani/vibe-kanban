# Prior Knowledge: Active MCP Refresh

The project knowledge base is not empty. The following established facts should
shape the specification and implementation plan.

## Relevant pages

### `docs/knowledge-base/mcp-connectivity-testing.md`

- VK historically writes executor-native MCP configuration; it does not own the
  coding agent's live MCP connections or tool inventory. The existing
  `mcp_test.rs` probe is a separate, on-demand MCP client, so a successful probe
  does not prove that a running agent adopted new tools.
- The reusable transport handshake is `initialize` →
  `notifications/initialized` → `tools/list`, with stdio, streamable HTTP, and
  legacy SSE-specific behavior already implemented.
- Native configurations are deliberately heterogeneous and must be normalized
  without mistaking unsupported shapes for failures.
- Diagnostics are untrusted and potentially secret-bearing. Existing rules cap
  previews, redact configured header values longest-first, omit HTML bodies,
  sanitize redirect locations, drain stdio stderr, and bound probes by timeout.
- Tool counts already exist in connectivity results. UI aggregation must not sum
  counts or treat unknown counts as zero.

### `docs/knowledge-base/shared-mcp-configuration.md`

- Executor-native files are the source of truth; there is no separate shared MCP
  registry. A live refresh therefore has to re-read the active executor's native
  configuration, not a stale shared-settings draft.
- Shared writes are atomic per native file (stage + rename, with `.bak`), but a
  multi-profile save may partially succeed. Refresh must scope itself to the
  selected live session/executor and report what that executor actually loaded.
- Transport compatibility differs by executor and by UI codec. Codex and Grok
  support stdio and streamable HTTP but not legacy SSE; adapters cannot assume
  one universal reload mechanism or config shape.
- OAuth gateway credentials are encrypted and API-visible values are redacted.
  Placeholder hydration is narrowly scoped and fails closed, so refresh must not
  copy or expose credential material while fingerprinting definitions.

### `docs/knowledge-base/forked-mcp-server-packaging.md`

- The pinned Slack fork is intentionally delivered from
  `davidvasandani/slack-mcp-server` release assets. The concrete release
  `v1.3.0-vk.2` is expected to expose attachment retrieval.
- A real verification must use an isolated cache and perform an MCP handshake
  through `tools/list`; catalog metadata alone proves nothing about the running
  binary.
- Diagnostics from MCP launchers belong on stderr because stdout is protocol
  framing. An argument-validation error from `attachment_get_data` proves the
  registered handler exists, while a generic unknown-tool error proves it does
  not.

### `docs/knowledge-base/grok-executor-integration.md`

- Grok runs as an ACP stdio agent and reads `~/.grok/config.toml`; its live
  session lifecycle is executor/vendor-owned.
- Generic `tool_output_error` events are not sufficient evidence of an MCP
  failure. Refresh verification must correlate the actual tool name, call, and
  result rather than infer from a broad vendor event category.
- Cross-executor behavior requires end-to-end wiring across executor
  implementation, generated types, frontend mappings, deployment services, and
  tests.

### `wiki/agent-process-lifecycle.md`

- A coding-agent turn and its `ExecutionProcess` are currently coupled, although
  selected executors can keep an app-server process warm across turns.
- `LocalContainerService` owns process handles and concurrency state. Long-lived
  OpenCode reuse is behind `VK_KEEP_WARM_AGENTS`; Codex and ACP live reuse are
  explicitly deferred because their transports are torn down at turn end.
- Any refresh implementation that claims same-session continuity must account
  for which agent process is actually alive between turns. A backend-only
  inventory cache cannot make a short-lived or transport-closed executor adopt
  tools.
- Existing concurrency lessons apply: publish/register before removing the old
  owner, never hold registry locks across process-kill awaits, use
  generation-conditional retirement, and keep clean cold-start fallback.

## Planning implications

1. Begin with executor capability discovery. The acceptance criteria cannot be
   met generically by the current connectivity probe or by rewriting config.
2. Separate the control-plane API/status model from executor-specific live
   reload adapters. Unsupported executors must be truthful.
3. Reuse transport normalization, handshake parsing, timeout, and redaction code
   from `mcp_test.rs` where it is applicable, but do not confuse a VK-owned probe
   inventory with the agent's active inventory.
4. Model refresh as immutable generations and retire old connections only after
   calls using them finish.
5. Test Slack against the pinned fork artifact and prove the live executor
   adopted `attachment_get_data`, not merely that an independent probe observed
   it.
6. Treat status timestamps as describing a confirmed published generation.
   Clear or retain them deliberately when configuration changes or a refresh
   partially fails.

## Risks and unresolved constraints

- The knowledge base documents no existing vendor-neutral live MCP reload
  command for Claude, Codex, Grok, or the other executors.
- Several executors are one-process-per-turn, and even warm-capable paths do not
  necessarily preserve their MCP client transport. The functional scope may
  need a first supported executor with capability-gated rollout.
- Resources and prompts are not part of the current connectivity result, so
  their enumeration and schema validation require extension.
