# Restart Agents After MCP Changes

## Legacy MCP identifier migration

Existing settings may contain display labels such as `Atlassian Rovo` as native
MCP server keys. New saves reject those keys while the unchanged-legacy
exception allows the invalid state to persist. Restart then rematerializes the
invalid key, and Codex omits or rejects the server.

Loading shared MCP configuration must derive a stable protocol-safe identifier
using the existing suggestion rule, retain the original key as the display
label, and preserve definitions, assignments, overrides, credentials, and
disabled state. Saving must atomically rename the key in every assigned profile
and update display-label metadata. Migration must fail without partial writes if
the suggested identifier collides or profiles disagree.

Acceptance requires `Atlassian Rovo` to become `atlassian_rovo` with its display
label preserved, collision and cross-profile disagreement coverage, and a
migrated Codex configuration that exposes the server after restart.

## Problem

The chat toolbar currently invokes a Codex-only live MCP reload. It does not
restart the agent process, and other executor types cannot use that contract, so
saved MCP changes may never become visible in the selected logical session.

## Required behavior

- Offer one executor-neutral “Restart agent for MCP changes” action.
- Determine running state from coding-agent processes belonging to the selected
  session, not from unrelated workspace processes.
- If the session is stopped, immediately launch a standard follow-up turn. The
  normal executor continuation path must create a fresh process and reload MCP
  configuration while preserving conversation history.
- If the session is running, show a confirmation dialog. Cancel does nothing.
  Confirm queues a follow-up using the existing session queue; the current turn
  finishes normally and finalization starts the queued turn in a fresh process.
- If a real user follow-up is already queued, preserve it: that next fresh turn
  already supplies the requested restart boundary.
- Use the same behavior for every coding-agent executor.

## Restart prompt

When no user follow-up already exists, use:

> MCP configuration changed. Continue the existing task using the refreshed
> tool configuration.

## Acceptance criteria

1. Stopped sessions start immediately with no dialog.
2. Running sessions do not mutate the queue until confirmation.
3. Cancel leaves the session unchanged.
4. Confirm queues one continuation and does not interrupt the active turn.
5. An existing queued user message is not replaced.
6. Running detection is session-scoped and coding-agent-only.
7. Focused tests cover immediate, cancel, queue, and preserve-existing cases.
8. No service outside Vibe Kanban is changed.
