# Grok executor integration

Contributing tasks: `43bc-add-grok-to-vk`, `ba9f-grok-vk-executor`,
`ffeb-debug-vk-mcp-err`

Grok Build integrates with Vibe Kanban through its ACP stdio mode. The executor
launches `grok --no-auto-update agent stdio`; model and automatic approval
options must precede the `agent stdio` subcommand. Follow-up turns use the
shared ACP transcript-replay mechanism under the `grok_sessions` namespace.

## Authentication and approvals

Grok advertises ACP authentication methods after initialisation. Try supported
methods in preference order and continue to the next advertised method when one
fails; a stale cached login must not prevent `XAI_API_KEY` authentication.
Authentication failures must complete the execution with a failure result and
an actionable `grok login`/API-key message.

Supervised mode is end-to-end wiring, not only an executor flag. Any executor
that passes an `ExecutorApprovalService` to the ACP client must also be included
in the deployment's `ExecutorApprovalBridge` selection. Otherwise the no-op
service silently approves every tool request. Automatic mode uses Grok's
`--always-approve` flag and does not attach interactive approvals.

Grok permission policy must also set the ACP session mode explicitly after
every `new_session` and before the prompt: Auto maps to `auto`, while
Supervised (including legacy profiles with no saved permission) maps to `ask`.
The CLI flag alone is insufficient because Grok's newly created ACP session can
otherwise report and behave as its default Ask mode. Keep these vendor-specific
mode IDs in the Grok executor and pass them through the provider-neutral ACP
harness; using the same configured harness for initial and follow-up turns
prevents continuation sessions from reverting.

## Native MCP shape

Grok reads TOML from `~/.grok/config.toml` under `mcp_servers`. It supports
stdio and typeless streamable HTTP entries:

- stdio: `command`, optional `args`, optional `env`;
- HTTP: `url`, optional `headers`, with no `type` key;
- legacy SSE is incompatible.

Apply those rules consistently in shared materialisation, preconfigured-server
adaptation, frontend codecs, conflict resolution, and new-server assignment
filters. Backend-only compatibility checks leave the UI able to construct saves
that the backend rejects.

## Diagnosing `tool_output_error`

Do not infer an MCP outage from Grok's `tool_output_error` label alone. Grok
uses that category for ordinary built-in tool failures too. Task
`ffeb-debug-vk-mcp-err` traced a reported `read_file` error to a parallel
Grok-native file read of a `SPEC.md` path before the file existed. The tool
returned a short “does not exist” result, Grok recorded one error beside three
successful reads, and the agent immediately continued, created the file, and
completed the task. The call never traversed Vibe Kanban's MCP server or failed
the ACP session.

Correlate before changing runtime code:

1. Use the logged session ID and timestamp to inspect Grok's retained
   `events.jsonl`, `chat_history.jsonl`, and `logs/unified.jsonl`.
2. Match the tool call ID to its arguments and result. Determine whether the
   name belongs to Grok's built-in tool catalog or an MCP server.
3. Check the events after the error: another inference loop and later successful
   calls prove recovery; an execution stop or lost session requires ACP-level
   investigation.
4. Keep diagnostics bounded and secret-safe. Tool metadata and a short error are
   useful; raw file outputs and credentials are not.

Do not blanket-suppress or retry these events. Missing-file results can be
intentional evidence, and the same category can still describe a real failure.
Change Vibe Kanban only when evidence shows it corrupted the protocol result or
promoted a non-fatal vendor event into a fatal execution state.

## Adding an executor across the product

Adding a base executor requires more than the Rust enum and implementation.
Register it with the type generator, commit the generated TypeScript and JSON
schema, add the default profile, provide frontend labels/icons and exhaustive
codec mappings, wire any deployment-level services such as approvals, and add
agent documentation/navigation. Validate with generated-type checks, focused
executor and codec tests, TypeScript checks, formatting, and a workspace build.
