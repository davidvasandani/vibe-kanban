# Prior knowledge: vk/5f70-not-loading-chat

Distilled from the knowledge bases (read-only): `vibe-kanban/wiki/` (INDEX
plus pages matching websocket/stream/history/mobile/loading) and
`homelab/docs/knowledge-base/` (vibe-kanban-* pages). Nothing in either
covers the chat conversation-history loader or `streamJsonPatchEntries`
directly, so this task starts that topic.

## Relevant pages

- `wiki/workspace-carousel-view.md` (carousel task): `WorkspacesMainContainer`
  and `ConversationList` are mounted per column outside the route, and each
  opens its own websockets, so the websocket count is bounded by mount
  windowing. For this task: any per-socket fix in `streamJsonPatchEntries`
  applies to every chat instance (route and carousel) at once, with no
  per-caller plumbing.
- `wiki/agent-process-lifecycle.md` (vk/1a64, vk/826e, vk/9f36): lost terminal
  events are a known class. The server reconciles them into `indeterminate`
  so the authoritative process stream clears stale UI. The same principle
  applies on the client side here: do not wait on a terminal signal that may
  never arrive.
- `wiki/mobile-kanban-scrolling.md`: mobile layout is a first-class surface.
  Reproduce it at phone width. A headless browser at 390 px CSS width renders
  the mobile tab layout.
- `homelab/docs/knowledge-base/vibe-kanban-public-mcp-access-routing.md`:
  `vibe.vasandani.dev` sits behind Cloudflare Access. From a worker, only
  `/mcp` is proxied with a service token (local Caddy route), so `/api` and
  websocket behaviour through the public edge cannot be exercised from
  here. Test against the coordinator at `172.16.100.102:3334` instead. The
  same page's gateway lesson carries over: status handling alone does not
  bound a peer that connects and then stalls. You need an explicit timeout,
  which here is the idle deadline.

## Constitution context

- XXXIX (0.35.0, vk/3fb0): request-scoped server reads must terminate on
  their own, and a sentinel that a filter can discard is no guarantee. This
  task adds the client-side counterpart (XL).
- XIX: live streams are bounded and self-correcting (retention and
  resnapshot). That is about retention, not settling a one-shot awaited read.

## Previous related change

- #323 (`ad96e84`): `/messages` returned only when a running execution ended,
  because termination depended on a `Finished` that a JsonPatch-only filter
  dropped. It was the same failure shape (waiting on a sentinel that never
  arrives), but server-side.
