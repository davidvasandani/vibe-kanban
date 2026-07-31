# Implementation Plan: Lazy-load workspace chat history

1. Establish the SpecKit constitution and feature workspace, then carry the
   requirements from `SPEC.md` and `PRIOR_KNOWLEDGE.md` into the canonical
   feature spec.
2. Clarify the transport boundary, page unit/size, live-handoff contract, and
   raw-script-log scope; record decisions as testable requirements.
3. Trace normalized log storage and `ContainerService::stream_normalized_logs`
   through local/remote implementations to choose a bounded materialization and
   cursor design that preserves cancellation and signed access.
4. Define contracts for a tail page, opaque older-page cursor, `has_more`,
   stable entry identity, and recoverable errors. Document the snapshot/live
   watermark and cursor validation rules.
5. Implement the backend read path so initial and older pages return bounded
   final normalized entries without replaying the entire transcript per page.
   Add authorization, limit caps, cancellation, ordering, add/replace/reset,
   cursor, and boundary tests.
6. Refactor `useConversationHistory` into independent bounded history paging and
   active live-stream reconciliation. Remove automatic background loading,
   expose `loadEarlier`, `hasEarlier`, and loading/error state, and reject stale
   results after scope changes.
7. Wire top-of-list demand loading into `ConversationListContainer`, preserving
   a semantic row/offset anchor across prepends and providing accessible
   loading, retry, and end-of-history states.
8. Add focused frontend tests for initial tail loading, no idle prefetch,
   single-flight top loading, prepend identity/order, live continuation,
   recoverable failure, and workspace/session switching.
9. Run formatting, focused tests, generated-type checks if contracts changed,
   and broader `pnpm run check`/lint/backend tests in proportion to the touched
   scope.
10. Run independent Codex diff review, address confirmed significant findings,
    and repeat verification/review until clean.
11. Distill the shipped pagination, cursor, live-handoff, and scroll-anchor
    lessons into the Vibe Kanban knowledge base, tag them with task
    `65ab-lazy-load-vk-wor`, refresh its index, and commit that knowledge update.
