# Feature Specification: Preserve Preview App Navigation URLs

**Feature dir**: `specs/vk/c89d-address-fable-fo/`
**Status**: Clarified

## Summary

Preserve the latest page selected inside a workspace's application preview so a
user returning to that preview resumes the same worksheet or application view,
instead of being returned to the detected development-server root. This makes
Vibe Kanban's displayed app URL a reliable representation of the previewed
application's current state.

## User Stories

- As a user previewing Mad Minutes, I want the worksheet I selected to remain
  selected after I leave and return to the preview so that I can continue work
  without finding it again.
- As a user, I want the preview URL bar, copy action, and open action to represent
  the same current application page so that links do not silently lose route
  state.
- As a user working in multiple workspaces, I want each workspace to remember
  only its own preview page so that navigation never leaks between tasks.

## Functional Requirements

- FR-1: The system must recognize an accepted navigation event from the embedded
  application as the current preview URL.
- FR-2: The current preview URL must preserve its pathname, query string, and URL
  fragment.
- FR-3: The system must retain the current preview URL when the preview UI is
  unmounted and later remounted for the same workspace.
- FR-4: Retained preview URLs must be scoped to one workspace.
- FR-5: Internal preview transport metadata must not appear in the retained or
  user-visible application URL.
- FR-6: A retained navigation URL must not change the established meaning or
  lifecycle of an explicit manual URL override.
- FR-7: A newly detected development server must remain able to establish the
  preview's origin and port while the retained application route restores the
  user's last in-app location when compatible.
- FR-8: Stale or duplicate navigation reports must not overwrite a newer retained
  URL.
- FR-9: Preview navigation, refresh, back, forward, copied URLs, and external-open
  actions must continue to operate on the same canonical current URL.

## Out of Scope

- Changes to Mad Minutes or any application other than Vibe Kanban.
- Synchronizing preview history across different workspaces or users.
- Persisting the full browser back/forward history across application sessions.
- Treating arbitrary cross-origin pages as injectable preview applications.

## Acceptance Criteria

- [ ] Navigating from the Mad Minutes root to a selected worksheet and remounting
      the preview restores that worksheet.
- [ ] A selected URL containing path, query, and fragment data retains all three.
- [ ] The preview URL bar, copy action, and open action expose the restored URL
      without `_refresh` or other Vibe Kanban transport parameters.
- [ ] Navigating workspace A does not change the restored preview URL of
      workspace B.
- [ ] Manual override behavior and development-server URL auto-detection remain
      functional.
- [ ] Duplicate or stale bridge events cannot replace the latest accepted route.
- [ ] Focused automated regression tests pass in both shared frontend usage
      contexts.

## Open Questions

No open questions remain.

## Clarifications

- The latest in-app URL is durable per-workspace state and must survive both a
  preview remount and a Vibe Kanban page reload.
- Auto-detected previews retain the selected route (pathname, query, and
  fragment) separately from the detected server origin. When the same workspace
  starts its app on a new compatible local origin or port, Vibe Kanban applies
  the retained route to that newly detected origin instead of pinning the stale
  full origin. Explicit manual overrides continue to retain their complete URL.
- A retained auto-detected route is considered compatible with the next
  auto-detected application for that same workspace; clearing or replacing an
  explicit override does not promote the override's route into auto-detected
  state.
