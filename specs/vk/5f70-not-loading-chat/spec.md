# Feature Specification: The chat panel always finishes loading

**Feature dir**: `specs/vk/5f70-not-loading-chat/`
**Status**: Clarified

## Summary

Opening a workspace's chat can leave the conversation area showing a spinner
indefinitely. It was reported from a phone going through the public
Cloudflare-fronted origin, on two different workspaces within the same minute,
while the coordinator was under heavy NFS I/O wait. Reloading later on the LAN
loaded both.

To show a conversation, the chat fetches the log of each recent completed turn
and waits for each fetch to report that it is finished. A fetch that ends
without saying so never tells the chat anything, and nothing times it out, so
the chat waits forever. The server does end fetches this way: when reading a
log fails, it closes the connection cleanly without the "finished" marker.
Proxies and mobile operating systems also drop connections silently.

We want every such fetch to end definitively. It either succeeds, or it fails
within a bounded time. When one fails, the chat shows whatever did load and
keeps the missing turn retryable instead of blocking the whole panel.

## User Stories

- As a user on my phone, I want a workspace's chat to show its messages (or a
  clear partial state) instead of spinning forever when one turn's history
  cannot be fetched.
- As a user, I want a turn that failed to load to stay reachable, so that I can
  ask for it again without reloading the app.
- As a user watching a live turn, I want the live view to reconnect when its
  connection is dropped, instead of freezing silently.

## Functional Requirements

- FR-1: A fetch of a completed turn's history MUST end as either success
  (all entries plus the finished marker) or failure. A connection that closes
  without the finished marker, even cleanly, counts as a failure.
- FR-2: A fetch of a completed turn's history that receives nothing for longer
  than 30 seconds MUST fail. Any message received resets the
  deadline, so a large history that is still arriving is not cut off.
- FR-3: Each fetch MUST report its outcome exactly once, even when several
  terminal signals arrive together (for example an error followed by a close).
- FR-4: When a fetch fails during the chat's initial load, the chat MUST still
  finish loading and render the turns that did load. The failed turn stays
  unloaded, so it remains reachable through the existing "load earlier"
  control.
- FR-5: A live turn's stream that closes without the finished marker MUST be
  treated as a dropped connection and retried with the existing bounded
  backoff. Live streams MUST NOT be subject to the idle deadline, because a
  running agent may be silent for long periods.
- FR-6: Closing a fetch deliberately (the user navigates away) MUST NOT be
  reported as a failure.

## Out of Scope

- Server-side changes to how log streams terminate or report errors.
- The workspace-list and execution-process streams, which already reconnect on
  close.
- Making history normalization faster under I/O pressure.
- Homelab deployment or Cloudflare configuration.

## Acceptance Criteria

- [ ] A history fetch whose connection closes cleanly without the finished
      marker reports failure once, and the chat's initial load completes.
- [ ] A history fetch that receives no messages within the idle deadline
      reports failure once; one that keeps receiving messages is not cut off.
- [ ] A history fetch that receives the finished marker reports success once,
      with no failure reported by the close that follows.
- [ ] An error followed by a close reports a single failure.
- [ ] Closing a fetch deliberately reports nothing.
- [ ] A live stream that closes without the finished marker is retried.
- [ ] `pnpm run check`, `pnpm run lint`, and the web-core vitest suite pass.

## Open Questions

None remain. Resolved in `clarifications.md`:

- Idle deadline for history fetches: **30 seconds**, reset on every message.
- Failed turns in the initial load are retried **on demand only**, through the
  existing "load earlier" control. They are not retried automatically.
