# Technical Specification: Authoritative Execution Status Reconciliation

Task: `vk/3488-fix-stale-execut`

## Problem

The session execution-process WebSocket reads its database snapshot before
subscribing to live broadcasts. A terminal update committed in that gap appears
in neither source, so the browser can retain a stale `running` process and keep
the chat composer on Stop indefinitely. Broadcast lag is also silently ignored,
creating a second missed-event path with no forced resnapshot.

## Required Behavior

- Subscribe to execution-process updates before capturing the full session
  snapshot.
- Emit the snapshot and Ready marker before draining buffered/live updates.
- Treat broadcast lag as loss of stream authority and close with a retryable
  error so the client reconnects and obtains a new snapshot.
- Retain the last good snapshot during ordinary reconnect downtime, then replace
  it with the new authoritative snapshot.
- Define only `running` coding-agent/setup/cleanup/archive processes as active.
  Completed, failed, killed, interrupted, and indeterminate statuses clear Stop.
- Preserve active Stop/cancellation behavior.

## Lifecycle Invariants

Coordinator-local non-persistent processes that cannot be adopted after restart
become interrupted after safe WIP handling. Worker-owned uncertainty remains
evidence-backed and may become indeterminate. Transport loss alone never
fabricates completion, but neither interrupted nor indeterminate is displayed as
an active cancellable turn.

## Verification

- A rendered hook regression retains running while disconnected and converges
  to interrupted when the reconnect snapshot arrives.
- Status derivation tests cover active running and every terminal status.
- The event stream tests its lag-to-resnapshot error contract.
- Existing shutdown cleanup coverage proves warm/local process teardown.
- Focused frontend tests, services tests, server compilation, TypeScript checks,
  formatting, and diff checks pass.
