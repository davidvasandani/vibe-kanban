# Grok executor integration

Contributing tasks: `43bc-add-grok-to-vk`, `ba9f-grok-vk-executor`

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

## Adding an executor across the product

Adding a base executor requires more than the Rust enum and implementation.
Register it with the type generator, commit the generated TypeScript and JSON
schema, add the default profile, provide frontend labels/icons and exhaustive
codec mappings, wire any deployment-level services such as approvals, and add
agent documentation/navigation. Validate with generated-type checks, focused
executor and codec tests, TypeScript checks, formatting, and a workspace build.
