# Implementation Plan: Reliable MCP Reload

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The affected UI is React/TypeScript in `packages/web-core`. It calls the
existing POST/GET refresh endpoints through `sessionsApi`. The Rust
`McpRefreshCoordinator` is already the session-scoped authority and deliberately
returns a transient `busy` view for duplicate requests while retaining the
canonical pending state.

## Architecture & Approach

Extract the refresh request, canonical status hydration, polling, session
isolation, and result messaging from
`packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx`
into a small feature-local hook. The container will remain responsible for
rendering the toolbar item, while the hook owns confirmed refresh-view state.

On every valid existing-session identity, the hook clears the prior session's
view and immediately reads canonical status. It ignores late responses by
checking the active session key. When POST returns `busy`, the hook immediately
reads canonical status instead of treating the transient projection as the
state to poll. Canonical `pending_next_turn` state drives the existing bounded
poll interval until a terminal result arrives.

No backend or API contract change is planned. This is the smallest fix that
restores the backend's existing authoritative handoff semantics.

## Data Model

See `./data-model.md`. No persistent schema changes are required.

## Contracts

See `./contracts/mcp-refresh-ui.md`. Existing HTTP routes and generated types
are unchanged.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- Principle II: the hook receives focused Vitest coverage for hydration,
  duplicate-busy reconciliation, polling, and session races.
- Principle III/VI: existing endpoints, types, coordinator behavior, and toolbar
  presentation are reused.
- Principle IV: stateful feature logic stays in `web-core`; no UI primitive is
  reimplemented.
- Principle XII: the backend remains authoritative and the client protects the
  async session handoff against stale responses.
- Principle XVII: only process-confirmed canonical results are shown as applied;
  no connectivity probe or inferred success is introduced.

No constitution deviation is required.

## Risks & Dependencies

- React Strict Mode may run mount effects twice in tests/development. Hydration
  must be read-only and safe to repeat.
- A status response may arrive after the selected session changes. All request
  paths need the same active-session guard.
- Toasts can duplicate if polling re-observes a terminal result. Notify only on
  a transition produced by the current request/poll path.
- Fake timers and async polling tests must not leak intervals.
