# Clarifications: Hide Workspace Context Bar on Mobile Layout

`/speckit.clarify` found no blocking open questions after comparing the
feature request, `spec.md`, `PRIOR_KNOWLEDGE.md`, and the current workspace UI
implementation.

## Resolved decisions

| Question | Decision | Evidence |
| --- | --- | --- |
| What is the highlighted mobile overlay? | It is the workspace context bar: optional workspace chat chrome for desktop action shortcuts. | `WorkspacesMainContainer` mounts `ContextBarContainer` as `contextBarContent` only when a workspace exists and `hideContextBar` is false. The carousel already passes `hideContextBar`, and the wiki records the context bar as suppressible because its actions are route-context dependent. |
| Which mobile signal controls this feature? | Hide the context bar whenever the responsive workspace layout is mobile. `useIsMobile()` is the source of truth for that layout. | `WorkspacesLayout` reads `useIsMobile()` and branches into the mobile workspace composition. `useIsMobile()` is backed by the existing `(max-width: 767px)` media query. |
| Should physical-device detection remain relevant? | Yes. Retain the existing real-mobile guard as defense in depth, but do not rely on it alone. | `ContextBarContainer` currently returns `null` only when `isRealMobileDevice()` is true, so the defect remains possible when responsive mobile is true and real-device detection is false. |
| Where should the visibility policy live? | Keep the policy at the workspace composition or web-core context-bar container boundary. Do not add device or responsive awareness to the presentational `packages/ui` `ContextBar`. | `ContextBarContainer` already prepares action state, position, and platform guards in `packages/web-core`; `packages/ui/src/components/ContextBar.tsx` is presentational and only receives render items, style, drag state, and mouse handlers. |
| Should mobile get a touch-draggable context bar instead? | No. Mobile should use existing mobile navigation rather than a replacement floating control. | The mobile `WorkspacesLayout` already provides separate tabs for workspaces, chat, changes, logs, preview, browser, and Git. `PRIOR_KNOWLEDGE.md` also records that adding a second touch/drag overlay is not the intended fix. |
| Should this change alter desktop behavior or persistence? | No. Desktop rendering, action definitions, mouse drag behavior, snap positions, and persisted context-bar position must remain unchanged. | `ContextBarContainer` delegates desktop positioning to `useContextBarPosition(containerRef)` and passes the existing drag handler into the presentational bar. The spec's scope is visibility only. |
| Does the mobile breakpoint change? | No. The feature must reuse the existing `useIsMobile()` breakpoint and not introduce a new threshold. | `useIsMobile()` defines the breakpoint as `max-width: 767px`, and `WorkspacesLayout` already uses that hook to select mobile layout. |
| What automated coverage is needed later? | Add focused coverage for the visibility decision, including the mismatch case `responsive mobile = true` and `physical mobile = false`, plus physical-mobile hidden and desktop-visible cases. | The repo already uses Vitest under `packages/web-core`, and the relevant rule can be tested without backend, database, API, or generated type changes. |

## Non-blocking implementation notes

- A small pure predicate for context-bar visibility would make the required
  signal-disagreement cases easy to test.
- If the implementation uses React hooks in `ContextBarContainer`, read the
  responsive hook unconditionally before returning `null` so hook ordering stays
  stable.
- Manual mobile verification remains useful because the repository's documented
  task environment does not provide a real touch engine.

## Remaining open questions

None. `spec.md` already matches these decisions, so no spec update was needed.
