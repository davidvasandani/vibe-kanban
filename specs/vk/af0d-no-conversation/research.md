# Research: Recover Missing Codex Conversations

## Pinned upstream behavior

The official `openai/codex` source at tag `rust-v0.144.1` implements
`thread/fork` by reading the source through `read_stored_thread_for_resume`.
`ThreadStoreError::ThreadNotFound` maps to JSON-RPC invalid-request code
`-32600`, message `no rollout found for thread id <uuid>`, and no data.

The reported production text, `No conversation found with session ID: <uuid>`,
does not exist at that source tag. It is nevertheless direct runtime evidence
from the failing deployment and describes the same absent persisted
conversation boundary. Supporting both exact complete templates is safer than
a generic `contains("not found")` rule.

## Decision: preserve structured RPC errors

`jsonrpc::await_response` currently wraps only the RPC message in
`ExecutorError::Io`, erasing code and data. Add a structured variant. This lets
the recovery require `-32600` and leaves future callers able to make similarly
defensive decisions without parsing a decorated I/O string.

Alternative rejected: match `ExecutorError::to_string()`. The label prefix and
I/O wrapper are presentation details, and substring matching could hide
unrelated invalid requests.

## Decision: recover normal chat only

The user action is a follow-up conversation. Reviews and slash commands have
different semantics; silently starting a context-free thread can make a review
or compaction appear meaningful when its source is absent. Leave those paths
fail-loud.

## Decision: reuse ordinary new-thread setup

Calling `thread/start` on the already initialized app-server preserves the same
workspace launch, configuration, model, permission, MCP, and authentication
boundary. The common code then registers the returned ID before starting the
turn, which is exactly the durable handoff needed for later follow-ups.

No new dependency is required.
