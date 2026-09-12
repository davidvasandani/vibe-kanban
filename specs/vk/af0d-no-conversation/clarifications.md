# Clarifications

## 1. Recovery scope

The fallback applies to ordinary Codex chat follow-ups, including automatic
resume prompts that use that same path. Review and slash-command operations are
semantically different and remain unchanged. This is the smallest scope that
directly satisfies the reported failure without turning every fork use into an
implicit new conversation.

## 2. Missing-conversation error contract

The pinned upstream source (`openai/codex`, tag `rust-v0.144.1`) returns JSON-RPC
invalid-request code `-32600` from `thread/fork` when the stored rollout is
absent. At that tag the message is `no rollout found for thread id <uuid>` and
`data` is absent. The reported deployed diagnostic is `No conversation found
with session ID: <uuid>`, showing that wording has varied across the executable
boundary.

Vibe Kanban currently collapses every JSON-RPC error into `ExecutorError::Io`,
discarding code and data. The implementation will preserve the structured RPC
error and classify only invalid-request responses whose complete normalized
message matches one of the verified absent-thread forms for a valid UUID. A
substring such as `thread not found` is too broad because the pinned app-server
uses it for operations against an unloaded live thread as well.

## 3. User-visible behavior

Recovery is transparent at submission time. The replacement thread emits its
normal session ID, so existing persistence makes it the next continuation
target. The visible Vibe Kanban transcript remains available, but no claim is
made that lost Codex-private context was restored.
