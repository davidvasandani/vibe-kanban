# Technical Spec: Recover Missing Codex Conversations

**Task:** `vk/af0d-no-conversation`

## Problem

When Vibe Kanban sends a follow-up to a Codex session, the executor forks the
persisted Codex conversation ID. If the local Codex rollout is no longer
available, Codex app-server returns `No conversation found with session ID:
<id>` and the follow-up fails. The user is left with an error even though the
Vibe Kanban workspace, its chat history, and the new prompt are still usable.

## Goal

Treat a missing resumable Codex conversation as a recoverable loss of executor
state: start a new Codex conversation in the same workspace and submit the
follow-up there, while retaining strict failure behavior for unrelated fork
errors.

## Requirements

1. A Codex follow-up first attempts the existing `thread/fork` behavior.
2. If and only if Codex reports that the requested conversation is absent, the
   executor starts a new thread using the same thread-start parameters and
   submits the current prompt to it.
3. The replacement thread is registered normally so its new external session
   ID is persisted and subsequent follow-ups resume it.
4. Authentication, configuration, model selection, approval/collaboration
   mode, working directory, prompt content, and MCP configuration are identical
   to an ordinary new thread in that workspace.
5. Other `thread/fork` failures remain visible and do not silently create a new
   conversation.
6. Review and slash-command semantics are unchanged unless their existing
   normal-chat path shares the narrowly scoped recovery helper safely.
7. Focused tests cover the exact missing-conversation classification, the
   fallback, successful forks, and non-matching errors.

## Technical direction

Preserve structured JSON-RPC error information through the Codex client instead
of relying on an unbounded user-facing string match where practical. Add a
narrow classifier for the upstream missing-conversation response and use it at
the normal chat fork boundary. Because thread-start parameters are consumed by
either fork or start, arrange ownership so the same parameters can be used by
the fallback without changing successful behavior.

## Acceptance criteria

- Reproducing a follow-up with a nonexistent Codex session ID creates a new
  Codex thread in the same workspace and runs the prompt.
- The emitted replacement session ID becomes the durable session used by the
  next follow-up.
- A genuine permission, protocol, configuration, or app-server failure still
  fails visibly.
- Focused Rust tests and the relevant executor checks pass.
- No service outside Vibe Kanban is changed. Deployment configuration in
  `homelab/modules/vibe-kanban-rebuild.nix` is changed only if the source fix
  requires it.

## Out of scope

- Reconstructing lost Codex-private conversation context.
- Changing other executors' resume behavior.
- Modifying unrelated homelab services.
