# Technical plan (/speckit.plan)

Spec: ./spec.md

## Evidence and approach
`useConversationHistory.ts` loads initial history once, streams running processes,
and reloads only observed running→terminal transitions. A follow-up first seen
terminal after reconnect is ignored forever by all three paths. It becomes
older history rather than current chat. Server discovery already snapshots and
subscribes without a gap; wakeups use ordinary follow-ups.

Extend the existing terminal reconciliation effect in
packages/web-core/src/features/workspace-chat/model/hooks/useConversationHistory.ts
to also read newly discovered terminal processes after initial loading. Use
previous observed IDs to distinguish these from intentionally unloaded history.
Retain status-transition refresh and scoped generation checks for async work. Failed terminal-first reads remain accessible via the existing load-earlier recovery control.
Guard live stream callbacks against prior scopes and removed processes as well.

## Tests
Add useConversationHistory.test.tsx beside the hook. Drive context snapshots
through an open hook: running follow-up stream, terminal-first follow-up,
initial pagination, session change during fetch and removal during fetch.
Mock transport boundaries, retain real history/cache behavior.

## Contracts and data model
No API, schema or persistence changes. Existing timeline source and stable keys
are retained. Fetching stays HTTP-first with settled fallback and concurrency
limits from existing history code.

## Constitution check
II: behavioral regression tests. III: reuse terminal reconciliation.
IV: shared web-core reaches both apps. XLIV: no extra discovery socket or polling.
No deviations or new dependencies.

## Risks
Concurrent initial reads and live callbacks can race; guards must check scope
and current membership at completion. Older unloaded IDs must never auto-fetch
merely because a new turn appeared. Verify both running and terminal paths.
